use http::Uri;

use super::{classify_control_route, headers};

#[test]
/// 已退役的支付、邮件和通知路径不再被识别为可用管理接口。
fn retired_admin_routes_are_not_classified() {
    let headers = headers(&[]);
    for (method, path) in [
        (http::Method::GET, "/api/admin/payments/orders"),
        (http::Method::GET, "/api/admin/monitoring/audit-logs"),
        (
            http::Method::POST,
            "/api/admin/payments/orders/order-1/credit/",
        ),
        (http::Method::PUT, "/api/admin/payments/gateways/epay"),
        (
            http::Method::POST,
            "/api/admin/payments/redeem-codes/batches",
        ),
        (http::Method::POST, "/api/admin/system/smtp/test"),
        (
            http::Method::POST,
            "/api/admin/system/important-notification/test",
        ),
        (http::Method::GET, "/api/admin/system/email/templates"),
        (
            http::Method::PUT,
            "/api/admin/system/email/templates/welcome",
        ),
        (
            http::Method::POST,
            "/api/admin/system/email/templates/welcome/preview",
        ),
        (
            http::Method::POST,
            "/api/admin/system/email/templates/welcome/reset",
        ),
    ] {
        let uri: Uri = path.parse().expect("valid test URI");
        assert!(
            classify_control_route(&method, &uri, &headers).is_none(),
            "retired route still classified: {method} {path}"
        );
    }
}

#[tokio::test]
/// 实际 Router 对退役路径使用既有未实现响应，并保留系统版本查询。
async fn retired_admin_routes_use_unsupported_fallback() {
    use crate::tests::{build_router_with_state, send_request, AppState};
    use axum::body::Body;

    let app = build_router_with_state(AppState::new().expect("gateway should build"));
    for (method, path) in [
        (http::Method::GET, "/api/admin/payments/orders"),
        (http::Method::GET, "/api/admin/monitoring/audit-logs"),
        (http::Method::POST, "/api/admin/system/smtp/test"),
        (http::Method::GET, "/api/admin/system/email/templates"),
    ] {
        let request = http::Request::builder()
            .method(method)
            .uri(path)
            .body(Body::empty())
            .expect("request should build");
        let response = send_request(app.clone(), request).await;
        assert_eq!(
            response.status(),
            http::StatusCode::NOT_IMPLEMENTED,
            "{path}"
        );
    }

    let request = http::Request::builder()
        .uri("/api/admin/system/version")
        .header(crate::constants::GATEWAY_HEADER, "rust-phase3b")
        .header(crate::constants::TRUSTED_ADMIN_USER_ID_HEADER, "test-admin")
        .header(crate::constants::TRUSTED_ADMIN_USER_ROLE_HEADER, "admin")
        .header(
            crate::constants::TRUSTED_ADMIN_SESSION_ID_HEADER,
            "test-session",
        )
        .body(Body::empty())
        .expect("request should build");
    let response = send_request(app, request).await;
    assert_eq!(response.status(), http::StatusCode::OK);
}
