use http::Uri;

use super::{classify_control_route, headers};

#[test]
fn classifies_admin_oauth_list_providers_as_admin_proxy_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/admin/oauth/providers?limit=20"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("admin_proxy"));
    assert_eq!(decision.route_family.as_deref(), Some("oauth_manage"));
    assert_eq!(decision.route_kind.as_deref(), Some("list_providers"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("admin:oauth")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_admin_oauth_get_provider_as_admin_proxy_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/admin/oauth/providers/linuxdo"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("admin_proxy"));
    assert_eq!(decision.route_family.as_deref(), Some("oauth_manage"));
    assert_eq!(decision.route_kind.as_deref(), Some("get_provider"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("admin:oauth")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_admin_oauth_upsert_provider_as_admin_proxy_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/admin/oauth/providers/linuxdo"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::PUT, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("admin_proxy"));
    assert_eq!(decision.route_family.as_deref(), Some("oauth_manage"));
    assert_eq!(decision.route_kind.as_deref(), Some("upsert_provider"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("admin:oauth")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_admin_oauth_delete_provider_as_admin_proxy_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/admin/oauth/providers/linuxdo"
        .parse()
        .expect("uri should parse");
    let decision = classify_control_route(&http::Method::DELETE, &uri, &headers)
        .expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("admin_proxy"));
    assert_eq!(decision.route_family.as_deref(), Some("oauth_manage"));
    assert_eq!(decision.route_kind.as_deref(), Some("delete_provider"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("admin:oauth")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_admin_oauth_test_provider_as_admin_proxy_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/admin/oauth/providers/linuxdo/test"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::POST, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("admin_proxy"));
    assert_eq!(decision.route_family.as_deref(), Some("oauth_manage"));
    assert_eq!(decision.route_kind.as_deref(), Some("test_provider"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("admin:oauth")
    );
    assert!(!decision.is_execution_runtime_candidate());
}
