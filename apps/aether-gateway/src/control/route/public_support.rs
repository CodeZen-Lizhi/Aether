use super::{classified, is_gemini_models_route, is_gemini_operation_route, ClassifiedRoute};

fn has_single_segment_after_prefix(path: &str, prefix: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    let Some(segment) = trimmed.strip_prefix(prefix) else {
        return false;
    };
    !segment.is_empty() && !segment.contains('/')
}

fn has_single_nested_suffix_after_prefix(path: &str, prefix: &str, suffix: &str) -> bool {
    let trimmed = path.trim_end_matches('/');
    let Some(rest) = trimmed.strip_prefix(prefix) else {
        return false;
    };
    let Some((segment, actual_suffix)) = rest.split_once('/') else {
        return false;
    };
    !segment.is_empty() && !segment.contains('/') && actual_suffix == suffix
}

pub(super) fn classify_public_support_route(
    method: &http::Method,
    normalized_path: &str,
    public_models_auth_signature: &str,
) -> Option<ClassifiedRoute> {
    if method == http::Method::GET && normalized_path == "/v1/models" {
        Some(classified(
            "public_support",
            "models",
            "list",
            public_models_auth_signature,
            false,
        ))
    } else if method == http::Method::GET
        && normalized_path.starts_with("/v1/models/")
        && !is_gemini_models_route(normalized_path)
        && !is_gemini_operation_route(normalized_path)
    {
        Some(classified(
            "public_support",
            "models",
            "detail",
            public_models_auth_signature,
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/v1beta/models" {
        Some(classified(
            "public_support",
            "models",
            "list",
            public_models_auth_signature,
            false,
        ))
    } else if method == http::Method::GET
        && normalized_path.starts_with("/v1beta/models/")
        && !is_gemini_models_route(normalized_path)
        && !is_gemini_operation_route(normalized_path)
    {
        Some(classified(
            "public_support",
            "models",
            "detail",
            public_models_auth_signature,
            false,
        ))
    } else if method == http::Method::GET
        && matches!(
            normalized_path,
            "/api/public/site-info"
                | "/api/public/providers"
                | "/api/public/models"
                | "/api/public/search/models"
                | "/api/public/stats"
                | "/api/public/global-models"
                | "/api/public/health/api-formats"
                | "/api/public/health/models"
                | "/api/public/health/related"
        )
    {
        let route_kind = match normalized_path {
            "/api/public/site-info" => "site_info",
            "/api/public/providers" => "providers",
            "/api/public/models" => "models",
            "/api/public/search/models" => "search_models",
            "/api/public/stats" => "stats",
            "/api/public/global-models" => "global_models",
            "/api/public/health/api-formats" => "health_api_formats",
            "/api/public/health/models" => "health_models",
            "/api/public/health/related" => "health_related",
            _ => "site_info",
        };
        Some(classified(
            "public_support",
            "public_catalog",
            route_kind,
            "public:catalog",
            false,
        ))
    } else if method == http::Method::GET && matches!(normalized_path, "/api/auth/settings") {
        let route_kind = match normalized_path {
            "/api/auth/settings" => "settings",
            _ => "settings",
        };
        Some(classified(
            "public_support",
            "auth_public",
            route_kind,
            "public:auth",
            false,
        ))
    } else if matches!(method, &http::Method::GET | &http::Method::POST)
        && matches!(
            normalized_path,
            "/api/auth/login" | "/api/auth/refresh" | "/api/auth/me" | "/api/auth/logout"
        )
    {
        let route_kind = match normalized_path {
            "/api/auth/login" => "login",
            "/api/auth/refresh" => "refresh",
            "/api/auth/me" => "me",
            "/api/auth/logout" => "logout",
            _ => "login",
        };
        Some(classified(
            "public_support",
            "auth",
            route_kind,
            "user:auth",
            false,
        ))
    } else if method == http::Method::GET
        && matches!(
            normalized_path,
            "/api/dashboard/stats"
                | "/api/dashboard/recent-requests"
                | "/api/dashboard/provider-status"
                | "/api/dashboard/daily-stats"
        )
    {
        let route_kind = match normalized_path {
            "/api/dashboard/stats" => "stats",
            "/api/dashboard/recent-requests" => "recent_requests",
            "/api/dashboard/provider-status" => "provider_status",
            "/api/dashboard/daily-stats" => "daily_stats",
            _ => "stats",
        };
        Some(classified(
            "public_support",
            "dashboard",
            route_kind,
            "user:dashboard",
            false,
        ))
    } else if method == http::Method::GET
        && matches!(
            normalized_path,
            "/api/users/me" | "/api/users/me/sessions" | "/api/users/me/preferences"
        )
    {
        let route_kind = match normalized_path {
            "/api/users/me" => "detail",
            "/api/users/me/sessions" => "sessions",
            "/api/users/me/preferences" => "preferences",
            _ => "detail",
        };
        Some(classified(
            "public_support",
            "users_me",
            route_kind,
            "user:self",
            false,
        ))
    } else if method == http::Method::PUT
        && matches!(
            normalized_path,
            "/api/users/me" | "/api/users/me/preferences"
        )
    {
        let route_kind = match normalized_path {
            "/api/users/me" => "update_detail",
            "/api/users/me/preferences" => "preferences_update",
            _ => "update_detail",
        };
        Some(classified(
            "public_support",
            "users_me",
            route_kind,
            "user:self",
            false,
        ))
    } else if method == http::Method::PATCH && normalized_path == "/api/users/me/password" {
        Some(classified(
            "public_support",
            "users_me",
            "password",
            "user:self",
            false,
        ))
    } else if method == http::Method::DELETE && normalized_path == "/api/users/me/sessions/others" {
        Some(classified(
            "public_support",
            "users_me",
            "sessions_others_delete",
            "user:self",
            false,
        ))
    } else if matches!(method, &http::Method::PATCH | &http::Method::DELETE)
        && has_single_segment_after_prefix(normalized_path, "/api/users/me/sessions/")
    {
        let route_kind = if method == http::Method::PATCH {
            "session_update"
        } else {
            "session_delete"
        };
        Some(classified(
            "public_support",
            "users_me",
            route_kind,
            "user:self",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/api/capabilities" {
        Some(classified(
            "public_support",
            "capabilities",
            "list",
            "public:capabilities",
            false,
        ))
    } else if method == http::Method::GET && normalized_path.starts_with("/api/capabilities/model/")
    {
        Some(classified(
            "public_support",
            "capabilities",
            "model",
            "public:capabilities",
            false,
        ))
    } else if method == http::Method::GET
        && matches!(
            normalized_path,
            "/" | "/health" | "/v1/health" | "/v1/providers" | "/v1/test-connection"
        )
    {
        let route_kind = match normalized_path {
            "/" => "root",
            "/health" | "/v1/health" => "health",
            "/v1/providers" => "providers",
            "/v1/test-connection" => "test_connection",
            _ => "root",
        };
        Some(classified(
            "public_support",
            "system_catalog",
            route_kind,
            "public:system_catalog",
            false,
        ))
    } else if method == http::Method::GET && normalized_path.starts_with("/v1/providers/") {
        Some(classified(
            "public_support",
            "system_catalog",
            "provider_detail",
            "public:system_catalog",
            false,
        ))
    } else if method == http::Method::GET && normalized_path == "/test-connection" {
        Some(classified(
            "public_support",
            "system_catalog",
            "test_connection",
            "public:system_catalog",
            false,
        ))
    } else {
        None
    }
}
