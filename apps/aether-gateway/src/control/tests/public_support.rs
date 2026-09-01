use http::Uri;

use super::{classify_control_route, headers, GatewayPublicRequestContext};
use crate::handlers::shared::local_proxy_route_requires_buffered_body;

#[test]
fn classifies_models_list_as_public_support_route() {
    let headers = headers(&[("authorization", "Bearer sk-test")]);
    let uri: Uri = "/v1/models".parse().expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("models"));
    assert_eq!(decision.route_kind.as_deref(), Some("list"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("openai:chat")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_codex_models_list_with_responses_auth_signature() {
    let headers = headers(&[("authorization", "Bearer sk-test")]);
    let uri: Uri = "/v1/models?client_version=0.144.1"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("models"));
    assert_eq!(decision.route_kind.as_deref(), Some("list"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("openai:responses")
    );
}

#[test]
fn empty_codex_client_version_uses_responses_signature_for_bounded_fallback() {
    let headers = headers(&[("authorization", "Bearer sk-test")]);
    let uri: Uri = "/v1/models?client_version="
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("openai:responses")
    );
}

#[test]
fn classifies_v1beta_models_as_gemini_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/v1beta/models?pageSize=10"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("models"));
    assert_eq!(decision.route_kind.as_deref(), Some("list"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("gemini:generate_content")
    );
}

#[test]
fn classifies_public_catalog_site_info_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/site-info".parse().expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("site_info"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_dashboard_stats_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/dashboard/stats".parse().expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("dashboard"));
    assert_eq!(decision.route_kind.as_deref(), Some("stats"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("user:dashboard")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_providers_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/providers?limit=20"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("providers"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_models_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/models?provider_id=provider-openai"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("models"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_search_models_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/search/models?q=gpt"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("search_models"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_stats_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/stats".parse().expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("stats"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_global_models_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/global-models?limit=10"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("global_models"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_health_api_formats_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/health/api-formats?lookback_hours=12"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("health_api_formats"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_health_models_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/health/models?lookback_hours=12"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("health_models"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_public_catalog_health_related_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/public/health/related?dimension=endpoint&value=openai%3Achat"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("public_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("health_related"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_auth_settings_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/auth/settings".parse().expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("auth_public"));
    assert_eq!(decision.route_kind.as_deref(), Some("settings"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:auth")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_auth_routes_as_public_support_route() {
    for (method, path, route_kind) in [
        (http::Method::POST, "/api/auth/login", "login"),
        (http::Method::POST, "/api/auth/refresh", "refresh"),
        (http::Method::GET, "/api/auth/me", "me"),
        (http::Method::POST, "/api/auth/logout", "logout"),
    ] {
        let headers = headers(&[]);
        let uri: Uri = path.parse().expect("uri should parse");
        let decision =
            classify_control_route(&method, &uri, &headers).expect("route should classify");

        assert_eq!(decision.route_class.as_deref(), Some("public_support"));
        assert_eq!(decision.route_family.as_deref(), Some("auth"));
        assert_eq!(decision.route_kind.as_deref(), Some(route_kind));
        assert_eq!(
            decision.auth_endpoint_signature.as_deref(),
            Some("user:auth")
        );
        assert!(!decision.is_execution_runtime_candidate());
    }
}

#[test]
fn classifies_capabilities_list_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/capabilities".parse().expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("capabilities"));
    assert_eq!(decision.route_kind.as_deref(), Some("list"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:capabilities")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_capabilities_model_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/api/capabilities/model/gpt-5"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("capabilities"));
    assert_eq!(decision.route_kind.as_deref(), Some("model"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:capabilities")
    );
    assert!(!decision.is_execution_runtime_candidate());
}

#[test]
fn classifies_system_catalog_provider_detail_as_public_support_route() {
    let headers = headers(&[]);
    let uri: Uri = "/v1/providers/provider-openai"
        .parse()
        .expect("uri should parse");
    let decision =
        classify_control_route(&http::Method::GET, &uri, &headers).expect("route should classify");

    assert_eq!(decision.route_class.as_deref(), Some("public_support"));
    assert_eq!(decision.route_family.as_deref(), Some("system_catalog"));
    assert_eq!(decision.route_kind.as_deref(), Some("provider_detail"));
    assert_eq!(
        decision.auth_endpoint_signature.as_deref(),
        Some("public:system_catalog")
    );
    assert!(!decision.is_execution_runtime_candidate());
}
