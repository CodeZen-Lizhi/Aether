use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use axum::{extract::ws::WebSocketUpgrade, routing::get, Router};
use clap::{CommandFactory, FromArgMatches, Parser};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Notify;
use tokio::time::timeout;

use super::*;

#[derive(Parser)]
struct TestArgs {
    #[command(flatten)]
    lifecycle: LifecycleArgs,
}

fn parse_options(args: &[&str]) -> Result<LifecycleArgs, clap::Error> {
    let command = TestArgs::command()
        .mut_arg("app_host", |arg| arg.env(None::<&str>))
        .mut_arg("shutdown_timeout_seconds", |arg| arg.env(None::<&str>));
    let matches = command.try_get_matches_from(args)?;
    Ok(TestArgs::from_arg_matches(&matches)?.lifecycle)
}

#[test]
fn cli_defaults_preserve_public_binding_and_do_not_watch_stdin() {
    let args = parse_options(&["gateway"]).unwrap();
    assert_eq!(args.app_host, IpAddr::V4(Ipv4Addr::UNSPECIFIED));
    assert_eq!(args.shutdown_timeout_seconds, 20);
    assert!(!args.exit_on_stdin_close);
}

#[test]
fn desktop_options_select_loopback_and_owned_process_shutdown() {
    let args = parse_options(&[
        "gateway",
        "--app-host",
        "127.0.0.1",
        "--shutdown-timeout-seconds",
        "30",
        "--exit-on-stdin-close",
    ])
    .unwrap();
    assert_eq!(args.app_host, IpAddr::V4(Ipv4Addr::LOCALHOST));
    assert_eq!(args.shutdown_timeout_seconds, 30);
    assert!(args.exit_on_stdin_close);
    assert!(parse_options(&["gateway", "--app-host", "invalid"]).is_err());
    assert!(parse_options(&["gateway", "--shutdown-timeout-seconds", "0"]).is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn stdin_watcher_ignores_bytes_and_waits_for_eof() {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let (mut parent, child) = UnixStream::pair().unwrap();
    let mut eof = spawn_eof_watcher(child).unwrap();
    parent.write_all(b"keep-alive\n").unwrap();
    assert!(matches!(
        eof.try_recv(),
        Err(oneshot::error::TryRecvError::Empty)
    ));
    drop(parent);
    timeout(Duration::from_secs(1), eof)
        .await
        .expect("closing the parent pipe must request shutdown")
        .unwrap()
        .unwrap();
}

#[cfg(unix)]
#[test]
fn open_stdin_watcher_does_not_prevent_tokio_runtime_exit() {
    use std::os::unix::net::UnixStream;

    let (parent, child) = UnixStream::pair().unwrap();
    let (done, completion) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async {
            let _watcher = spawn_eof_watcher(child).unwrap();
            tokio::task::yield_now().await;
        });
        drop(runtime);
        done.send(()).unwrap();
    });
    completion
        .recv_timeout(Duration::from_secs(1))
        .expect("signal-driven shutdown must not wait for the parent to close stdin");
    drop(parent);
    worker.join().unwrap();
}

async fn launch(
    router: Router,
    grace: Duration,
) -> (
    SocketAddr,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<ShutdownOutcome>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = oneshot::channel();
    let server = tokio::spawn(serve_gateway_router(
        vec![listener],
        router,
        128,
        async move {
            stopped.await.unwrap();
            Ok(ShutdownReason::Signal)
        },
        grace,
    ));
    (addr, stop, server)
}

async fn wait_until_listener_closed(addr: SocketAddr) {
    timeout(Duration::from_secs(1), async {
        loop {
            if TcpStream::connect(addr).await.is_err() {
                return;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("shutdown must stop accepting new connections");
}

async fn active_request_finishes_before_exit(http2: bool) {
    let entered = Arc::new(Notify::new());
    let release = Arc::new(Notify::new());
    let route_entered = Arc::clone(&entered);
    let route_release = Arc::clone(&release);
    let router = Router::new().route(
        "/",
        get(move || {
            let entered = Arc::clone(&route_entered);
            let release = Arc::clone(&route_release);
            async move {
                entered.notify_one();
                release.notified().await;
                "request completed"
            }
        }),
    );
    let (addr, stop, server) = launch(router, Duration::from_secs(2)).await;
    let builder = reqwest::Client::builder()
        .no_proxy()
        .tls_built_in_root_certs(false);
    let client = if http2 {
        builder.http2_prior_knowledge()
    } else {
        builder.http1_only()
    }
    .build()
    .unwrap();
    let request = tokio::spawn(async move {
        client
            .get(format!("http://{addr}/"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap()
    });
    timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    stop.send(()).unwrap();
    wait_until_listener_closed(addr).await;
    assert!(
        !server.is_finished(),
        "in-flight request must retain the server"
    );
    release.notify_one();
    assert_eq!(request.await.unwrap(), "request completed");
    let outcome = timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    outcome.result.unwrap();
    assert!(outcome.connections_drained);
}

#[tokio::test]
async fn graceful_shutdown_preserves_active_http1_request() {
    active_request_finishes_before_exit(false).await;
}

#[tokio::test]
async fn graceful_shutdown_preserves_active_http2_request() {
    active_request_finishes_before_exit(true).await;
}

#[tokio::test]
async fn graceful_shutdown_preserves_sse_body_until_completion() {
    let release = Arc::new(Notify::new());
    let route_release = Arc::clone(&release);
    let router = Router::new().route(
        "/",
        get(move || {
            let release = Arc::clone(&route_release);
            async move {
                let stream = async_stream::stream! {
                    yield Ok::<_, io::Error>("data: first\n\n");
                    release.notified().await;
                    yield Ok::<_, io::Error>("data: [DONE]\n\n");
                };
                (
                    [("content-type", "text/event-stream")],
                    Body::from_stream(stream),
                )
            }
        }),
    );
    let (addr, stop, server) = launch(router, Duration::from_secs(2)).await;
    let mut response = reqwest::Client::builder()
        .no_proxy()
        .tls_built_in_root_certs(false)
        .build()
        .unwrap()
        .get(format!("http://{addr}/"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.chunk().await.unwrap().unwrap(), "data: first\n\n");
    stop.send(()).unwrap();
    wait_until_listener_closed(addr).await;
    assert!(!server.is_finished());
    release.notify_one();
    assert_eq!(response.text().await.unwrap(), "data: [DONE]\n\n");
    assert!(server.await.unwrap().connections_drained);
}

async fn stalled_request_times_out(http2: bool) {
    let entered = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let route_entered = Arc::clone(&entered);
    let route_dropped = Arc::clone(&dropped);
    let router = Router::new().route(
        "/",
        get(move || {
            let entered = Arc::clone(&route_entered);
            let dropped = Arc::clone(&route_dropped);
            async move {
                let _request = Dropped(dropped);
                entered.notify_one();
                std::future::pending::<&'static str>().await
            }
        }),
    );
    let (addr, stop, server) = launch(router, Duration::from_millis(200)).await;
    let request = tokio::spawn(async move {
        let builder = reqwest::Client::builder()
            .no_proxy()
            .tls_built_in_root_certs(false);
        let client = if http2 {
            builder.http2_prior_knowledge()
        } else {
            builder.http1_only()
        }
        .build()
        .unwrap();
        client.get(format!("http://{addr}/")).send().await
    });
    timeout(Duration::from_secs(1), entered.notified())
        .await
        .unwrap();
    let shutdown_started = Instant::now();
    stop.send(()).unwrap();
    let outcome = timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    outcome.result.unwrap();
    assert!(!outcome.connections_drained);
    assert!(shutdown_started.elapsed() >= Duration::from_millis(100));
    assert!(timeout(Duration::from_secs(1), request)
        .await
        .unwrap()
        .unwrap()
        .is_err());
    timeout(Duration::from_secs(1), async {
        while !dropped.load(Ordering::Acquire) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("forced shutdown must drop the in-flight request future");
    assert!(TcpStream::connect(addr).await.is_err());
}

#[tokio::test]
async fn shutdown_timeout_cancels_a_stalled_request() {
    stalled_request_times_out(false).await;
}

#[tokio::test]
async fn shutdown_timeout_cancels_a_stalled_http2_request() {
    stalled_request_times_out(true).await;
}

async fn read_http_headers(socket: &mut TcpStream) -> String {
    timeout(Duration::from_secs(1), async {
        let mut headers = Vec::new();
        while !headers.ends_with(b"\r\n\r\n") {
            headers.push(socket.read_u8().await.unwrap());
            assert!(headers.len() < 16_384);
        }
        String::from_utf8(headers).unwrap()
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn shutdown_closes_idle_keep_alive_without_waiting_for_timeout() {
    let (addr, stop, server) = launch(
        Router::new().route("/", get(|| async { "ok" })),
        Duration::from_secs(3),
    )
    .await;
    let mut socket = TcpStream::connect(addr).await.unwrap();
    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\n\r\n")
        .await
        .unwrap();
    assert!(read_http_headers(&mut socket)
        .await
        .starts_with("HTTP/1.1 200"));
    let mut body = [0_u8; 2];
    socket.read_exact(&mut body).await.unwrap();
    assert_eq!(&body, b"ok");
    stop.send(()).unwrap();
    let outcome = timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    assert!(outcome.connections_drained);
    assert_eq!(socket.read(&mut body).await.unwrap(), 0);
}

async fn websocket_fixture(
    grace: Duration,
) -> (
    TcpStream,
    oneshot::Sender<()>,
    tokio::task::JoinHandle<ShutdownOutcome>,
    Arc<Notify>,
) {
    let finished = Arc::new(Notify::new());
    let route_finished = Arc::clone(&finished);
    let router = Router::new().route(
        "/",
        get(move |ws: WebSocketUpgrade| {
            let finished = Arc::clone(&route_finished);
            async move {
                ws.on_upgrade(move |mut socket| async move {
                    let _ = socket.recv().await;
                    finished.notify_one();
                })
            }
        }),
    );
    let (addr, stop, server) = launch(router, grace).await;
    let mut socket = TcpStream::connect(addr).await.unwrap();
    socket
        .write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Version: 13\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\r\n")
        .await
        .unwrap();
    assert!(read_http_headers(&mut socket)
        .await
        .starts_with("HTTP/1.1 101"));
    (socket, stop, server, finished)
}

#[tokio::test]
async fn shutdown_tracks_websocket_after_http_upgrade_completes() {
    let (mut socket, stop, server, finished) = websocket_fixture(Duration::from_secs(2)).await;
    stop.send(()).unwrap();
    wait_until_listener_closed(socket.peer_addr().unwrap()).await;
    assert!(
        !server.is_finished(),
        "upgraded socket must retain the server"
    );
    socket.write_all(&[0x88, 0x80, 0, 0, 0, 0]).await.unwrap();
    timeout(Duration::from_secs(1), finished.notified())
        .await
        .unwrap();
    assert!(server.await.unwrap().connections_drained);
}

#[tokio::test]
async fn shutdown_timeout_closes_idle_upgraded_websocket() {
    let (mut socket, stop, server, finished) = websocket_fixture(Duration::from_millis(200)).await;
    stop.send(()).unwrap();
    let outcome = timeout(Duration::from_secs(1), server)
        .await
        .unwrap()
        .unwrap();
    assert!(!outcome.connections_drained);
    timeout(Duration::from_secs(1), finished.notified())
        .await
        .expect("force close must wake the detached WebSocket reader");
    let mut buffer = [0_u8; 1];
    assert_eq!(socket.read(&mut buffer).await.unwrap(), 0);
}

#[tokio::test]
async fn shutdown_error_still_releases_every_listener() {
    let first = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let second = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addresses = [first.local_addr().unwrap(), second.local_addr().unwrap()];
    let outcome = serve_gateway_router(
        vec![first, second],
        Router::new(),
        128,
        async { Err(io::Error::other("shutdown signal unavailable")) },
        Duration::from_secs(1),
    )
    .await;
    assert!(outcome.result.is_err());
    assert!(outcome.connections_drained);
    for addr in addresses {
        assert!(TcpStream::connect(addr).await.is_err());
    }
}

struct Dropped(Arc<AtomicBool>);

impl Drop for Dropped {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Release);
    }
}

#[tokio::test]
async fn shutdown_cancels_initialization_before_workers_start() {
    let entered = Arc::new(Notify::new());
    let dropped = Arc::new(AtomicBool::new(false));
    let startup_entered = Arc::clone(&entered);
    let startup_dropped = Arc::clone(&dropped);
    run_with_shutdown(
        async move {
            entered.notified().await;
            Ok(ShutdownReason::StdinClosed)
        },
        move |_shutdown| async move {
            let _guard = Dropped(startup_dropped);
            startup_entered.notify_one();
            std::future::pending().await
        },
    )
    .await
    .unwrap();
    assert!(dropped.load(Ordering::Acquire));
}

#[tokio::test]
async fn shutdown_waits_for_cleanup_after_serving_begins() {
    let entered = Arc::new(Notify::new());
    let cleaned = Arc::new(AtomicBool::new(false));
    let startup_entered = Arc::clone(&entered);
    let startup_cleaned = Arc::clone(&cleaned);
    run_with_shutdown(
        async move {
            entered.notified().await;
            Ok(ShutdownReason::StdinClosed)
        },
        move |shutdown| async move {
            shutdown.begin_serving();
            startup_entered.notify_one();
            assert!(matches!(
                shutdown.requested().await?,
                ShutdownReason::StdinClosed
            ));
            tokio::task::yield_now().await;
            startup_cleaned.store(true, Ordering::Release);
            Ok(())
        },
    )
    .await
    .unwrap();
    assert!(cleaned.load(Ordering::Acquire));
}
