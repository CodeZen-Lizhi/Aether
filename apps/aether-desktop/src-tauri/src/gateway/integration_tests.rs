use std::{fs, net::TcpStream, path::PathBuf, sync::mpsc};

use security_framework::{
    os::macos::keychain::SecKeychain,
    passwords::{delete_generic_password, get_generic_password},
};

use super::*;

struct Fixture {
    gateway: Arc<Gateway>,
    _directory: tempfile::TempDir,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.gateway.stop();
        let _ = delete_generic_password(
            &secrets::service_name(&self.gateway.paths.data),
            "gateway-secrets",
        );
    }
}

fn capability(gateway: &Gateway) -> String {
    gateway
        .inner()
        .unwrap()
        .process
        .as_ref()
        .unwrap()
        .session
        .secret()
        .to_string()
}

fn client() -> reqwest::blocking::Client {
    reqwest::blocking::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn login(gateway: &Gateway) -> String {
    let status = gateway.status(false).unwrap();
    assert_eq!(status.phase, Phase::Running);
    assert!(status.configured && status.pid.is_some());
    let response = client()
        .post(format!("{}/_gateway/desktop/session", status.gateway_url))
        .header("Origin", &status.gateway_url)
        .header("X-Aether-Desktop-Session", capability(gateway))
        .header("X-Client-Device-Id", "aether-desktop-host-integration")
        .send()
        .unwrap();
    assert!(response.status().is_success());
    let cookie = response.headers()[reqwest::header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    assert!(cookie.contains("HttpOnly"));
    let cookie = cookie.split(';').next().unwrap();
    // The normal Cookie refresh contract remains available to the WebView.
    let refresh = client()
        .post(format!("{}/api/auth/refresh", status.gateway_url))
        .header("Cookie", cookie)
        .header("X-Client-Device-Id", "aether-desktop-host-integration")
        .send()
        .unwrap();
    assert!(refresh.status().is_success());
    response.json::<serde_json::Value>().unwrap()["access_token"]
        .as_str()
        .unwrap()
        .to_string()
}

fn assert_session(gateway: &Gateway, token: &str) {
    let status = gateway.status(false).unwrap();
    assert_eq!(status.phase, Phase::Running);
    let response = client()
        .get(format!("{}/api/auth/me", status.gateway_url))
        .header("X-Client-Device-Id", "aether-desktop-host-integration")
        .bearer_auth(token)
        .send()
        .unwrap();
    assert!(response.status().is_success());
}

fn restart_while_opening(gateway: &Arc<Gateway>) {
    let (opened, opening) = mpsc::sync_channel(0);
    let (release, released) = mpsc::sync_channel(0);
    let opening_gateway = gateway.clone();
    let window = thread::spawn(move || {
        opening_gateway.with_dashboard(|connection| {
            opened.send(connection.generation).unwrap();
            released.recv_timeout(Duration::from_secs(5)).unwrap();
            Ok(())
        })
    });
    let previous_generation = opening.recv_timeout(Duration::from_secs(5)).unwrap();
    let (begin, begun) = mpsc::sync_channel(0);
    let (done, completed) = mpsc::channel();
    let restarting_gateway = gateway.clone();
    let restart = thread::spawn(move || {
        begin.send(()).unwrap();
        let result = restarting_gateway.restart();
        done.send(()).unwrap();
        result
    });
    begun.recv_timeout(Duration::from_secs(5)).unwrap();
    assert!(completed.recv_timeout(Duration::from_millis(100)).is_err());
    release.send(()).unwrap();
    window.join().unwrap().unwrap();
    restart.join().unwrap().unwrap();
    let next_generation = gateway
        .with_dashboard(|connection| Ok(connection.generation))
        .unwrap();
    assert_ne!(previous_generation, next_generation);
}

#[test]
#[ignore = "Requires bundled gateway/web paths and a disposable macOS Keychain item"]
fn native_gateway_passwordless_restart_and_missing_keys_preserve_data() {
    let _interaction = SecKeychain::disable_user_interaction().unwrap();
    let directory = tempfile::tempdir().unwrap();
    let paths = Paths::new(
        directory.path().join("Desktop 数据#1"),
        PathBuf::from(std::env::var_os("AETHER_DESKTOP_QA_GATEWAY").expect("bundled gateway path"))
            .canonicalize()
            .unwrap(),
        PathBuf::from(std::env::var_os("AETHER_DESKTOP_QA_WEB").expect("bundled web path"))
            .canonicalize()
            .unwrap(),
    )
    .unwrap();
    let gateway = Gateway::new(paths.clone());
    let mut fixture = Fixture {
        gateway: gateway.clone(),
        _directory: directory,
    };
    let occupied = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = occupied.local_addr().unwrap().port();
    gateway.set_port(port).unwrap();
    assert!(gateway.start().unwrap_err().contains("已被占用"));
    assert!(!paths.database.exists());
    drop(occupied);

    gateway.start().unwrap();
    let original_capability = capability(&gateway);
    let token = login(&gateway);
    let unauthenticated = client()
        .get(format!(
            "{}/api/auth/me",
            gateway.status(false).unwrap().gateway_url
        ))
        .header("X-Client-Device-Id", "aether-desktop-host-integration")
        .send()
        .unwrap();
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert!(gateway.set_port(port).is_err());
    let service = secrets::service_name(&paths.data);
    let original_keys = get_generic_password(&service, "gateway-secrets").unwrap();
    let settings = fs::read_to_string(&paths.settings).unwrap();
    assert!(!settings.contains("password") && !settings.contains(&original_capability));
    assert!(!gateway
        .logs
        .lock()
        .unwrap()
        .iter()
        .any(|line| line.contains(&original_capability)));
    gateway.stop().unwrap();
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());

    let restored = Gateway::new(paths.clone());
    fixture.gateway = restored.clone();
    restored.start().unwrap();
    assert_ne!(original_capability, capability(&restored));
    let old_capability = client()
        .post(format!(
            "{}/_gateway/desktop/session",
            restored.status(false).unwrap().gateway_url
        ))
        .header("Origin", restored.status(false).unwrap().gateway_url)
        .header("X-Aether-Desktop-Session", original_capability)
        .header("X-Client-Device-Id", "aether-desktop-host-integration")
        .send()
        .unwrap();
    assert_eq!(old_capability.status(), reqwest::StatusCode::UNAUTHORIZED);
    assert_session(&restored, &token);
    assert!(original_keys == get_generic_password(&service, "gateway-secrets").unwrap());
    restart_while_opening(&restored);
    assert_session(&restored, &token);
    // A revoked/expired browser session can be replaced without a password.
    assert!(client()
        .post(format!(
            "{}/api/auth/logout",
            restored.status(false).unwrap().gateway_url
        ))
        .bearer_auth(&token)
        .header("X-Client-Device-Id", "aether-desktop-host-integration")
        .send()
        .unwrap()
        .status()
        .is_success());
    assert_session(&restored, &login(&restored));
    restored.stop().unwrap();
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());

    let database = fs::read(&paths.database).unwrap();
    delete_generic_password(&service, "gateway-secrets").unwrap();
    assert!(restored.start().unwrap_err().contains("缺少原加密密钥"));
    assert_eq!(restored.status(false).unwrap().phase, Phase::Failed);
    assert_eq!(
        get_generic_password(&service, "gateway-secrets")
            .unwrap_err()
            .code(),
        -25300
    );
    assert!(database == fs::read(&paths.database).unwrap());
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, port)).is_err());
}
