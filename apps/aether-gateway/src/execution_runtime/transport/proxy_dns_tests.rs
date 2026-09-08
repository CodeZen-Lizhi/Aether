//! Exercise the configured proxy through the real HTTP and WebSocket clients.
//! `socks5h` must send the hostname to the proxy; `socks5` keeps local DNS.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::time::Duration;

use aether_contracts::{ExecutionTimeouts, ProxySnapshot, ResolvedTransportProfile};
use base64::Engine;
use serde_json::json;
use sha1::{Digest, Sha1};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;

use super::{
    build_browser_wreq_client, build_direct_reqwest_client_from_cache_key,
    direct_reqwest_client_cache_key, resolve_proxy_url, ExecutionTransportControls,
};
use crate::ai_serving::AiExecutionDecision;
use crate::handlers::proxy::{
    connect_upstream_websocket, UpstreamWebSocketErrorCodes, RESPONSES_WEBSOCKET_SESSION_LIMITS,
};

const TEST_TIMEOUT: Duration = Duration::from_secs(5);
const TARGET_PORT: u16 = 65535;
const REMOTE_HOST: &str = "provider-dns.invalid";

#[derive(Debug)]
enum SocksTarget {
    Ip(IpAddr),
    Domain(String),
}

#[derive(Debug)]
struct ProxyRequest {
    target: SocksTarget,
    port: u16,
    http_headers: String,
}

async fn start_proxy(websocket: bool) -> (SocketAddr, JoinHandle<ProxyRequest>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .expect("bind local SOCKS fixture");
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        tokio::time::timeout(TEST_TIMEOUT, async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            assert_eq!(socket.read_u8().await.unwrap(), 5);
            let method_count = socket.read_u8().await.unwrap();
            let mut methods = vec![0; usize::from(method_count)];
            socket.read_exact(&mut methods).await.unwrap();
            assert!(methods.contains(&0), "fixture uses no authentication");
            socket.write_all(&[5, 0]).await.unwrap();

            let mut header = [0; 4];
            socket.read_exact(&mut header).await.unwrap();
            assert_eq!(&header[..3], &[5, 1, 0]);
            let target = match header[3] {
                1 => {
                    let mut octets = [0; 4];
                    socket.read_exact(&mut octets).await.unwrap();
                    SocksTarget::Ip(Ipv4Addr::from(octets).into())
                }
                3 => {
                    let length = socket.read_u8().await.unwrap();
                    let mut hostname = vec![0; usize::from(length)];
                    socket.read_exact(&mut hostname).await.unwrap();
                    SocksTarget::Domain(String::from_utf8(hostname).unwrap())
                }
                4 => {
                    let mut octets = [0; 16];
                    socket.read_exact(&mut octets).await.unwrap();
                    SocksTarget::Ip(Ipv6Addr::from(octets).into())
                }
                other => panic!("unexpected SOCKS address type {other}"),
            };
            let port = socket.read_u16().await.unwrap();
            socket
                .write_all(&[5, 0, 0, 1, 127, 0, 0, 1, 0, 0])
                .await
                .unwrap();

            // Read exactly the handshake so the fixture never consumes frames.
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                assert!(request.len() < 16 * 1024, "bounded HTTP handshake");
                request.push(socket.read_u8().await.unwrap());
            }
            let http_headers = String::from_utf8(request).unwrap();
            if websocket {
                let key = http_headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(name, _)| name.eq_ignore_ascii_case("sec-websocket-key"))
                    .map(|(_, value)| value.trim())
                    .expect("WebSocket client sends its handshake key");
                let accept = base64::engine::general_purpose::STANDARD.encode(Sha1::digest(
                    format!("{key}258EAFA5-E914-47DA-95CA-C5AB0DC85B11"),
                ));
                let response = format!(
                    "HTTP/1.1 101 Switching Protocols\r\nConnection: Upgrade\r\nUpgrade: websocket\r\nSec-WebSocket-Accept: {accept}\r\nX-Proxy-Fixture: socks\r\n\r\n"
                );
                socket.write_all(response.as_bytes()).await.unwrap();
            } else {
                socket
                    .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                    .await
                    .unwrap();
            }
            ProxyRequest {
                target,
                port,
                http_headers,
            }
        })
        .await
        .expect("local proxy handshake must finish within its budget")
    });
    (address, task)
}

fn proxy_snapshot(scheme: &str, address: SocketAddr) -> ProxySnapshot {
    ProxySnapshot {
        enabled: Some(true),
        url: Some(format!("{scheme}://{address}")),
        ..ProxySnapshot::default()
    }
}

fn browser_profile() -> ResolvedTransportProfile {
    ResolvedTransportProfile {
        profile_id: "chrome136".to_string(),
        backend: "browser_wreq".to_string(),
        http_mode: "http1_only".to_string(),
        ..ResolvedTransportProfile::default()
    }
}

fn timeouts() -> ExecutionTimeouts {
    ExecutionTimeouts {
        connect_ms: Some(2_000),
        total_ms: Some(2_000),
        ..ExecutionTimeouts::default()
    }
}

fn assert_dns_mode(request: ProxyRequest, scheme: &str, hostname: &str) {
    match (scheme, request.target) {
        ("socks5h", SocksTarget::Domain(domain)) => assert_eq!(domain, hostname),
        ("socks5", SocksTarget::Ip(ip)) => assert!(ip.is_loopback()),
        ("socks5", SocksTarget::Domain(address)) => {
            // reqwest 0.12.28/hyper-util 0.1.20 encode a locally resolved IPv6
            // address as a bracketed numeric domain. The original hostname
            // must still be resolved locally, never passed to the proxy.
            let numeric_address = address
                .strip_prefix('[')
                .and_then(|address| address.strip_suffix(']'))
                .unwrap_or(&address);
            let ip: IpAddr = numeric_address
                .parse()
                .expect("socks5 must resolve the hostname before contacting the proxy");
            assert!(ip.is_loopback());
        }
        (_, target) => panic!("{scheme} used an unexpected DNS mode: {target:?}"),
    }
    assert_eq!(request.port, TARGET_PORT);
    assert!(request
        .http_headers
        .starts_with("GET /dns-probe HTTP/1.1\r\n"));
    assert!(request
        .http_headers
        .to_ascii_lowercase()
        .contains(&format!("host: {hostname}:{TARGET_PORT}\r\n")));
}

async fn assert_http_proxy_dns(browser: bool) {
    for (scheme, hostname) in [("socks5h", REMOTE_HOST), ("socks5", "localhost")] {
        let (address, server) = start_proxy(false).await;
        let proxy = proxy_snapshot(scheme, address);
        let upstream_url = format!("http://{hostname}:{TARGET_PORT}/dns-probe");
        let body = tokio::time::timeout(TEST_TIMEOUT, async {
            if browser {
                let client = build_browser_wreq_client(
                    Some(&timeouts()),
                    Some(&proxy),
                    &browser_profile(),
                    ExecutionTransportControls::default(),
                    true,
                )
                .expect("build the existing browser client");
                let response = client.get(&upstream_url).send().await.unwrap();
                assert_eq!(response.status().as_u16(), 200);
                response.text().await.unwrap()
            } else {
                let cache_key = direct_reqwest_client_cache_key(
                    &upstream_url,
                    "proxy-dns-test",
                    Some(&timeouts()),
                    resolve_proxy_url(Some(&proxy)).unwrap(),
                    None,
                    ExecutionTransportControls::default(),
                );
                let client = build_direct_reqwest_client_from_cache_key(&cache_key).unwrap();
                let response = client.get(&upstream_url).send().await.unwrap();
                assert_eq!(response.status().as_u16(), 200);
                response.text().await.unwrap()
            }
        })
        .await
        .expect("HTTP request must finish through the local proxy");
        assert_eq!(body, "ok");
        assert_dns_mode(server.await.unwrap(), scheme, hostname);
    }
}

async fn assert_websocket_proxy_dns(browser: bool) {
    let errors = UpstreamWebSocketErrorCodes {
        upstream_url_missing: "url_missing",
        upstream_url_invalid: "url_invalid",
        frontdoor_self_loop: "self_loop",
        headers_invalid: "headers_invalid",
        client_build_failed: "client_build_failed",
        proxy_invalid: "proxy_invalid",
        tunnel_proxy_unsupported: "tunnel_unsupported",
        handshake_failed: "handshake_failed",
        upgrade_rejected: "upgrade_rejected",
        upgrade_failed: "upgrade_failed",
    };
    for (scheme, hostname) in [("socks5h", REMOTE_HOST), ("socks5", "localhost")] {
        let (address, server) = start_proxy(true).await;
        let decision: AiExecutionDecision = serde_json::from_value(json!({
            "action": "proxy",
            "upstream_url": format!("http://{hostname}:{TARGET_PORT}/dns-probe"),
            "proxy": proxy_snapshot(scheme, address),
            "timeouts": timeouts(),
            "transport_profile": browser.then(browser_profile),
        }))
        .unwrap();
        let connection = tokio::time::timeout(
            TEST_TIMEOUT,
            connect_upstream_websocket(&decision, RESPONSES_WEBSOCKET_SESSION_LIMITS, errors),
        )
        .await
        .expect("WebSocket handshake must finish through the local proxy")
        .expect("existing WebSocket transport must use the configured SOCKS DNS mode");
        assert_eq!(
            connection
                .response_headers
                .get("x-proxy-fixture")
                .map(String::as_str),
            Some("socks")
        );
        drop(connection);
        assert_dns_mode(server.await.unwrap(), scheme, hostname);
    }
}

#[tokio::test]
async fn socks_proxy_http_preserves_local_and_remote_dns() {
    assert_http_proxy_dns(false).await;
}

#[tokio::test]
async fn socks_proxy_browser_http_preserves_local_and_remote_dns() {
    assert_http_proxy_dns(true).await;
}

#[tokio::test]
async fn socks_proxy_websocket_preserves_local_and_remote_dns() {
    assert_websocket_proxy_dns(false).await;
}

#[tokio::test]
async fn socks_proxy_browser_websocket_preserves_local_and_remote_dns() {
    assert_websocket_proxy_dns(true).await;
}
