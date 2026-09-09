//! Process-owned listener and connection shutdown for the gateway binary.

use std::future::Future;
use std::io::{self, Read};
use std::net::{IpAddr, Shutdown};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use axum::{body::Body, extract::Request};
use hyper::body::Incoming;
use hyper_util::{
    rt::{TokioExecutor, TokioIo},
    server::conn::auto::Builder as HyperServerBuilder,
    service::TowerToHyperService,
};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinSet;
use tokio::time::{timeout_at, Instant};
use tokio_util::sync::{CancellationToken, WaitForCancellationFutureOwned};
use tower::{Service as _, ServiceExt as _};
use tracing::{info, warn};

pub const DEFAULT_SHUTDOWN_TIMEOUT_SECONDS: u64 = 20;

#[derive(clap::Args, Debug)]
pub struct LifecycleArgs {
    /// Interface to bind. Desktop hosts should explicitly select 127.0.0.1.
    #[arg(long, env = "APP_HOST", default_value = "0.0.0.0")]
    pub app_host: IpAddr,

    /// Total grace period for connections, usage persistence, and background tasks.
    #[arg(
        long,
        env = "AETHER_GATEWAY_SHUTDOWN_TIMEOUT_SECONDS",
        default_value_t = DEFAULT_SHUTDOWN_TIMEOUT_SECONDS,
        value_parser = clap::value_parser!(u64).range(1..=3_600)
    )]
    pub shutdown_timeout_seconds: u64,

    /// Exit when the owning process closes its piped stdin. Disabled for normal CLI use.
    #[arg(long, default_value_t = false)]
    pub exit_on_stdin_close: bool,
}

#[cfg(test)]
impl Default for LifecycleArgs {
    fn default() -> Self {
        Self {
            app_host: IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED),
            shutdown_timeout_seconds: DEFAULT_SHUTDOWN_TIMEOUT_SECONDS,
            exit_on_stdin_close: false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum ShutdownReason {
    Signal,
    StdinClosed,
}

pub struct ShutdownListener {
    serving: Arc<AtomicBool>,
    requested: oneshot::Receiver<io::Result<ShutdownReason>>,
}

impl ShutdownListener {
    /// Set immediately before starting workers, with no await between this
    /// transition and entering the server's graceful shutdown loop.
    pub fn begin_serving(&self) {
        self.serving.store(true, Ordering::Release);
    }

    pub async fn requested(self) -> io::Result<ShutdownReason> {
        self.requested
            .await
            .map_err(|_| io::Error::other("gateway shutdown listener closed unexpectedly"))?
    }
}

/// Observe signals and parent EOF during initialization as well as serving.
/// Before workers exist, cancelling initialization is enough. Once serving
/// begins, forward the same request and await the normal cleanup chain.
pub async fn run_with_shutdown<F, S>(
    shutdown: F,
    start: impl FnOnce(ShutdownListener) -> S,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: Future<Output = io::Result<ShutdownReason>>,
    S: Future<Output = Result<(), Box<dyn std::error::Error>>>,
{
    let serving = Arc::new(AtomicBool::new(false));
    let (request, requested) = oneshot::channel();
    let gateway = start(ShutdownListener {
        serving: Arc::clone(&serving),
        requested,
    });
    tokio::pin!(gateway);
    tokio::select! {
        result = &mut gateway => result,
        reason = shutdown => {
            if serving.load(Ordering::Acquire) {
                let _ = request.send(reason);
                gateway.await
            } else {
                let reason = reason?;
                info!(event_name = "gateway_startup_cancelled", ?reason, "gateway initialization cancelled by shutdown");
                Ok(())
            }
        }
    }
}

/// An absolute budget prevents each shutdown phase from restarting the timeout.
pub struct ShutdownOutcome {
    pub result: io::Result<()>,
    pub deadline: Instant,
    pub usage_deadline: Instant,
    pub connections_drained: bool,
}

pub async fn wait_for_gateway_shutdown(exit_on_stdin_close: bool) -> io::Result<ShutdownReason> {
    if !exit_on_stdin_close {
        aether_runtime::wait_for_shutdown_signal().await?;
        return Ok(ShutdownReason::Signal);
    }

    let eof = spawn_eof_watcher(io::stdin())?;
    tokio::select! {
        result = aether_runtime::wait_for_shutdown_signal() => {
            result?;
            Ok(ShutdownReason::Signal)
        }
        result = eof => {
            result.map_err(|_| io::Error::other("gateway stdin watcher stopped unexpectedly"))??;
            Ok(ShutdownReason::StdinClosed)
        }
    }
}

fn spawn_eof_watcher<R>(mut reader: R) -> io::Result<oneshot::Receiver<io::Result<()>>>
where
    R: Read + Send + 'static,
{
    let (sender, receiver) = oneshot::channel();
    // Tokio stdin uses an uncancellable blocking-pool read. A dedicated OS
    // thread must not keep Tokio runtime shutdown waiting on an open stdin.
    std::thread::Builder::new()
        .name("aether-parent-stdin".to_string())
        .spawn(move || {
            let mut buffer = [0_u8; 1_024];
            let result = loop {
                match reader.read(&mut buffer) {
                    Ok(0) => break Ok(()),
                    Ok(_) => {}
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                    Err(error) => break Err(error),
                }
            };
            let _ = sender.send(result);
        })?;
    Ok(receiver)
}

pub async fn serve_gateway_router<F>(
    listeners: Vec<TcpListener>,
    router: axum::Router,
    http2_max_concurrent_streams: u32,
    shutdown: F,
    shutdown_timeout: Duration,
) -> ShutdownOutcome
where
    F: Future<Output = io::Result<ShutdownReason>>,
{
    let stop_accepting = CancellationToken::new();
    let force_close = CancellationToken::new();
    // Each underlying socket owns a receiver, even after Hyper transfers it
    // to an Axum WebSocket task. Waiting only on HTTP connection futures
    // would incorrectly report those upgraded connections as finished.
    let (connections, connection_guard) = watch::channel(());
    let mut servers = JoinSet::new();
    for listener in listeners {
        servers.spawn(serve_gateway_listener(
            listener,
            router.clone(),
            http2_max_concurrent_streams,
            stop_accepting.clone(),
            force_close.clone(),
            connection_guard.clone(),
        ));
    }
    drop(connection_guard);

    let result = if servers.is_empty() {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "gateway requires at least one listener",
        ))
    } else {
        tokio::select! {
            result = shutdown => {
                match result {
                    Ok(reason) => {
                        info!(event_name = "gateway_shutdown_requested", ?reason, "gateway shutdown requested");
                        Ok(())
                    }
                    Err(error) => Err(error),
                }
            }
            result = servers.join_next() => listener_result(result),
        }
    };

    let started = Instant::now();
    let deadline = started + shutdown_timeout;
    let persistence_reserve = (shutdown_timeout / 10).min(Duration::from_secs(2));
    let background_reserve = (shutdown_timeout / 40).min(Duration::from_millis(500));
    let connection_deadline = deadline - persistence_reserve;
    let usage_deadline = deadline - background_reserve;
    stop_accepting.cancel();

    let mut result = result;
    let drained = timeout_at(connection_deadline, async {
        while let Some(listener) = servers.join_next().await {
            if let Err(error) = listener_result(Some(listener)) {
                if result.is_ok() {
                    result = Err(error);
                }
            }
        }
        connections.closed().await;
    })
    .await
    .is_ok();

    if !drained {
        warn!(
            event_name = "gateway_shutdown_connections_timed_out",
            remaining_connections = connections.receiver_count(),
            grace_seconds = shutdown_timeout.as_secs_f64(),
            "gateway connection grace expired; closing remaining connections before usage drain"
        );
        force_close.cancel();
        servers.abort_all();
        // Give cancelled HTTP drivers and upgraded socket readers a turn to
        // release their response bodies and submit terminal usage work.
        let _ = timeout_at(usage_deadline, async {
            while servers.join_next().await.is_some() {}
            connections.closed().await;
        })
        .await;
    }

    ShutdownOutcome {
        result,
        deadline,
        usage_deadline,
        connections_drained: drained,
    }
}

fn listener_result(
    result: Option<Result<io::Result<()>, tokio::task::JoinError>>,
) -> io::Result<()> {
    match result {
        Some(Ok(result)) => result,
        Some(Err(error)) => Err(io::Error::other(format!(
            "gateway listener task failed: {error}"
        ))),
        None => Ok(()),
    }
}

async fn serve_gateway_listener(
    listener: TcpListener,
    router: axum::Router,
    http2_max_concurrent_streams: u32,
    stop_accepting: CancellationToken,
    force_close: CancellationToken,
    connection_guard: watch::Receiver<()>,
) -> io::Result<()> {
    let mut make_service = router.into_make_service_with_connect_info::<std::net::SocketAddr>();
    loop {
        let (io, remote_addr) = tokio::select! {
            biased;
            _ = stop_accepting.cancelled() => return Ok(()),
            connection = listener.accept() => connection?,
        };
        let tower_service = make_service
            .call(remote_addr)
            .await
            .unwrap_or_else(|error| match error {})
            .map_request(|request: Request<Incoming>| request.map(Body::new));
        let hyper_service = TowerToHyperService::new(tower_service);
        let io = TokioIo::new(TrackedIo {
            io,
            _guard: connection_guard.clone(),
            cancelled: Box::pin(force_close.clone().cancelled_owned()),
            closed: false,
        });
        let shutdown = stop_accepting.clone();
        let force_close = force_close.clone();

        tokio::spawn(async move {
            let mut builder = HyperServerBuilder::new(TokioExecutor::new());
            builder.http2().enable_connect_protocol();
            builder
                .http2()
                .max_concurrent_streams(http2_max_concurrent_streams);
            let connection = builder.serve_connection_with_upgrades(io, hyper_service);
            tokio::pin!(connection);
            let result = tokio::select! {
                result = &mut connection => result,
                _ = shutdown.cancelled() => {
                    connection.as_mut().graceful_shutdown();
                    tokio::select! {
                        result = &mut connection => result,
                        _ = force_close.cancelled() => return,
                    }
                }
            };
            if let Err(error) = result {
                tracing::trace!(error = ?error, "gateway connection closed with error");
            }
        });
    }
}

struct TrackedIo {
    io: TcpStream,
    _guard: watch::Receiver<()>,
    cancelled: Pin<Box<WaitForCancellationFutureOwned>>,
    closed: bool,
}

impl TrackedIo {
    fn poll_forced_close(&mut self, cx: &mut Context<'_>) -> bool {
        if !self.closed && self.cancelled.as_mut().poll(cx).is_ready() {
            // Shutdown both halves so a WebSocket peer also observes closure,
            // rather than retaining an idle upgraded TCP connection.
            let _ = socket2::SockRef::from(&self.io).shutdown(Shutdown::Both);
            self.closed = true;
        }
        self.closed
    }
}

impl AsyncRead for TrackedIo {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        if self.poll_forced_close(cx) {
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.io).poll_read(cx, buffer)
    }
}

impl AsyncWrite for TrackedIo {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<io::Result<usize>> {
        if self.poll_forced_close(cx) {
            return Poll::Ready(Err(io::ErrorKind::ConnectionAborted.into()));
        }
        Pin::new(&mut self.io).poll_write(cx, buffer)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffers: &[io::IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        if self.poll_forced_close(cx) {
            return Poll::Ready(Err(io::ErrorKind::ConnectionAborted.into()));
        }
        Pin::new(&mut self.io).poll_write_vectored(cx, buffers)
    }

    fn is_write_vectored(&self) -> bool {
        self.io.is_write_vectored()
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.poll_forced_close(cx) {
            return Poll::Ready(Err(io::ErrorKind::ConnectionAborted.into()));
        }
        Pin::new(&mut self.io).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        if self.poll_forced_close(cx) {
            return Poll::Ready(Ok(()));
        }
        Pin::new(&mut self.io).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests;
