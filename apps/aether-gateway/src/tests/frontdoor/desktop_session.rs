use std::sync::Arc;

use axum::body::{to_bytes, Body};
use axum::http::{header, HeaderValue, Method, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use serde_json::{json, Value};

use aether_data::repository::auth::InMemoryAuthApiKeySnapshotRepository;
use aether_data::repository::users::StoredUserAuthRecord;

use crate::tests::send_request;
use crate::{build_router_with_state, AppState, DesktopSessionConfig};

const SESSION_PATH: &str = "/_gateway/desktop/session";
const AUTHORITY: &str = "127.0.0.1:8084";
const ORIGIN: &str = "http://127.0.0.1:8084";
const SECRET: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const DEVICE: &str = "desktop-session-device";

async fn desktop_gateway() -> (Router, AppState) {
    let owner = StoredUserAuthRecord::new(
        "desktop-owner".to_string(),
        None,
        true,
        "existing-owner".to_string(),
        Some("preserved-legacy-password-hash".to_string()),
        "admin".to_string(),
        "local".to_string(),
        None,
        None,
        None,
        true,
        false,
        Some(chrono::Utc::now()),
        None,
    )
    .unwrap();
    let state = AppState::new().unwrap().with_auth_users_for_tests([owner]);
    let state = state
        .with_desktop_session(
            DesktopSessionConfig::new(SECRET, AUTHORITY.parse().unwrap(), true).unwrap(),
        )
        .await
        .unwrap();
    (build_router_with_state(state.clone()), state)
}

fn session_request() -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(SESSION_PATH)
        .header(header::HOST, AUTHORITY)
        .header(header::ORIGIN, ORIGIN)
        .header("x-aether-desktop-session", SECRET)
        .header("x-client-device-id", DEVICE)
        .body(Body::empty())
        .unwrap()
}

fn session_count(state: &AppState) -> usize {
    state
        .auth_session_store
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .len()
}

async fn json_body(response: Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn authenticate(router: &Router) -> (Value, String) {
    let response = send_request(router.clone(), session_request()).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let set_cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_string();
    assert!(set_cookie.contains("HttpOnly"));
    assert!(set_cookie.contains("Path=/api/auth"));
    let cookie = set_cookie.split(';').next().unwrap().to_string();
    let payload = json_body(response).await;
    assert!(!payload.to_string().contains(SECRET));
    assert_eq!(payload["user_id"], "desktop-owner");
    assert_eq!(payload["role"], "admin");
    assert!(payload["access_token"]
        .as_str()
        .is_some_and(|token| !token.is_empty()));
    (payload, cookie)
}

#[tokio::test]
async fn desktop_session_endpoint_is_disabled_for_default_web_gateway() {
    let state = AppState::new().unwrap();
    let response = send_request(build_router_with_state(state.clone()), session_request()).await;
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert_eq!(session_count(&state), 0);
    assert!(state
        .auth_user_store
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn desktop_session_rejects_missing_wrong_and_duplicate_capabilities() {
    let (router, state) = desktop_gateway().await;
    for value in [
        None,
        Some(""),
        Some("short"),
        Some("ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff"),
    ] {
        let mut request = session_request();
        request.headers_mut().remove("x-aether-desktop-session");
        if let Some(value) = value {
            request.headers_mut().insert(
                "x-aether-desktop-session",
                HeaderValue::from_str(value).unwrap(),
            );
        }
        let response = send_request(router.clone(), request).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    let mut duplicate = session_request();
    duplicate
        .headers_mut()
        .append("x-aether-desktop-session", HeaderValue::from_static(SECRET));
    assert_eq!(
        send_request(router, duplicate).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(session_count(&state), 0);
}

#[tokio::test]
async fn desktop_session_rejects_cross_origin_host_rebinding_and_duplicate_origins() {
    let (router, state) = desktop_gateway().await;
    for (name, value) in [
        ("origin", None),
        ("origin", Some("null")),
        ("origin", Some("https://example.com")),
        ("origin", Some("http://localhost:8084")),
        ("origin", Some("http://127.0.0.1:8085")),
        ("origin", Some("http://127.0.0.1:8084/")),
        ("host", None),
        ("host", Some("localhost:8084")),
        ("host", Some("127.0.0.1:8085")),
        ("host", Some("127.1:8084")),
        ("host", Some("attacker.example:8084")),
    ] {
        let mut request = session_request();
        request.headers_mut().remove(name);
        if let Some(value) = value {
            request
                .headers_mut()
                .insert(name, HeaderValue::from_str(value).unwrap());
        }
        request
            .headers_mut()
            .insert("x-forwarded-host", HeaderValue::from_static(AUTHORITY));
        request
            .headers_mut()
            .insert("x-forwarded-proto", HeaderValue::from_static("http"));
        let response = send_request(router.clone(), request).await;
        assert_eq!(
            response.status(),
            StatusCode::FORBIDDEN,
            "{name}: {value:?}"
        );
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    for (name, value) in [("host", AUTHORITY), ("origin", ORIGIN)] {
        let mut request = session_request();
        request
            .headers_mut()
            .append(name, HeaderValue::from_static(value));
        assert_eq!(
            send_request(router.clone(), request).await.status(),
            StatusCode::FORBIDDEN
        );
    }
    assert_eq!(session_count(&state), 0);
}

#[tokio::test]
async fn desktop_session_requires_existing_device_id_contract() {
    let (router, state) = desktop_gateway().await;
    for value in [None, Some("not a device")] {
        let mut request = session_request();
        request.headers_mut().remove("x-client-device-id");
        if let Some(value) = value {
            request
                .headers_mut()
                .insert("x-client-device-id", HeaderValue::from_str(value).unwrap());
        }
        assert_eq!(
            send_request(router.clone(), request).await.status(),
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(session_count(&state), 0);
}

#[tokio::test]
async fn desktop_session_ignores_request_user_id_and_keeps_normal_management_authentication() {
    let (router, state) = desktop_gateway().await;
    let mut request = session_request();
    *request.uri_mut() = format!("{SESSION_PATH}?user_id=foreign-user")
        .parse()
        .unwrap();
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    *request.body_mut() =
        Body::from(json!({"user_id": "foreign-user", "role": "admin"}).to_string());
    let response = send_request(router.clone(), request).await;
    assert_eq!(response.status(), StatusCode::OK);
    let session = json_body(response).await;
    assert_eq!(session["user_id"], "desktop-owner");
    assert_eq!(session_count(&state), 1);
    assert_eq!(
        state
            .find_user_auth_by_id("desktop-owner")
            .await
            .unwrap()
            .unwrap()
            .password_hash
            .as_deref(),
        Some("preserved-legacy-password-hash")
    );

    for path in ["/api/admin/providers", "/api/auth/me"] {
        let mut request = session_request();
        *request.method_mut() = Method::GET;
        *request.uri_mut() = path.parse().unwrap();
        assert_eq!(
            send_request(router.clone(), request).await.status(),
            StatusCode::UNAUTHORIZED
        );
    }
    let me = Request::builder()
        .uri("/api/auth/me")
        .header(
            header::AUTHORIZATION,
            format!("Bearer {}", session["access_token"].as_str().unwrap()),
        )
        .header("x-client-device-id", DEVICE)
        .body(Body::empty())
        .unwrap();
    assert_eq!(send_request(router, me).await.status(), StatusCode::OK);
}

#[tokio::test]
async fn desktop_session_capability_is_not_a_proxy_api_key() {
    let (_, state) = desktop_gateway().await;
    let repository = Arc::new(InMemoryAuthApiKeySnapshotRepository::default());
    let state = state.with_data_state_for_tests(
        crate::data::GatewayDataState::with_auth_api_key_reader_for_tests(repository.clone()),
    );
    assert!(state.has_auth_api_key_reader());
    let router = build_router_with_state(state);
    let mut request = session_request();
    *request.uri_mut() = "/v1/chat/completions".parse().unwrap();
    request.headers_mut().insert(
        header::AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {SECRET}")).unwrap(),
    );
    request
        .headers_mut()
        .insert("x-api-key", HeaderValue::from_static(SECRET));
    request.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    *request.body_mut() = Body::from(
        json!({"model": "test", "messages": [{"role": "user", "content": "test"}]}).to_string(),
    );
    assert_eq!(
        send_request(router, request).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert!(repository.key_hash_lookup_count(&super::hash_api_key(SECRET)) > 0);
}

#[tokio::test]
async fn desktop_session_refreshes_and_reauthenticates_after_refresh_revocation() {
    let (router, state) = desktop_gateway().await;
    let (first, cookie) = authenticate(&router).await;
    let refresh_request = || {
        Request::builder()
            .method(Method::POST)
            .uri("/api/auth/refresh")
            .header(header::COOKIE, &cookie)
            .header("x-client-device-id", DEVICE)
            .body(Body::empty())
            .unwrap()
    };
    let refreshed = send_request(router.clone(), refresh_request()).await;
    assert_eq!(refreshed.status(), StatusCode::OK);
    assert!(json_body(refreshed).await["access_token"].is_string());
    state
        .revoke_user_session(
            "desktop-owner",
            first["session_id"].as_str().unwrap(),
            chrono::Utc::now(),
            "desktop-session-test",
        )
        .await
        .unwrap();
    assert_eq!(
        send_request(router.clone(), refresh_request())
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let (second, _) = authenticate(&router).await;
    assert_ne!(first["session_id"], second["session_id"]);
    assert_eq!(session_count(&state), 2);
}

#[tokio::test]
async fn desktop_session_refuses_a_user_disabled_after_startup() {
    let (router, state) = desktop_gateway().await;
    state
        .auth_user_store
        .as_ref()
        .unwrap()
        .lock()
        .unwrap()
        .get_mut("desktop-owner")
        .unwrap()
        .is_active = false;
    let response = send_request(router, session_request()).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert_eq!(session_count(&state), 0);
}
