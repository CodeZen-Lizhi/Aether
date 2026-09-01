use super::{
    build_auth_error_response, build_auth_wallet_summary_payload,
    decrypt_catalog_secret_with_fallbacks, encrypt_catalog_secret_with_fallbacks, handle_auth_me,
    query_param_optional_bool, query_param_value, resolve_authenticated_local_user,
    sanitize_public_model_config_for_user, unix_secs_to_rfc3339, AppState,
    AuthenticatedLocalUserContext, GatewayPublicRequestContext, PUBLIC_CAPABILITY_DEFINITIONS,
};
use crate::handlers::shared::{
    admin_stats_bad_request_response, parse_bounded_u32, round_to, AdminStatsTimeRange,
    AdminStatsUsageFilter,
};

const USERS_ME_AVAILABLE_MODELS_FETCH_LIMIT: usize = 1000;

pub(crate) fn base_url_from_request(
    headers: &http::HeaderMap,
    request_context: &GatewayPublicRequestContext,
) -> String {
    if let Some(value) = std::env::var("AETHER_PUBLIC_BASE_URL")
        .ok()
        .or_else(|| std::env::var("PUBLIC_BASE_URL").ok())
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| value.starts_with("https://") || value.starts_with("http://"))
    {
        return value;
    }

    let host = crate::headers::header_value_str(headers, "x-forwarded-host")
        .or_else(|| request_context.host_header.clone())
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| {
            !value.is_empty()
                && !value.contains('/')
                && !value.contains('\\')
                && !value.contains('@')
                && !value.contains(char::is_whitespace)
        })
        .unwrap_or_else(|| "localhost".to_string());
    let proto = crate::headers::header_value_str(headers, "x-forwarded-proto")
        .map(|value| value.trim().trim_end_matches(':').to_ascii_lowercase())
        .filter(|value| value == "http" || value == "https")
        .unwrap_or_else(|| "http".to_string());
    format!("{proto}://{host}")
}

#[path = "user_me_preferences.rs"]
mod user_me_preferences;
use user_me_preferences::*;
#[path = "user_me_profile.rs"]
mod user_me_profile;
use user_me_profile::*;
#[path = "user_me_sessions.rs"]
mod user_me_sessions;
use user_me_sessions::*;
#[path = "user_me_shared.rs"]
mod user_me_shared;
use user_me_shared::*;
#[path = "user_me_routes.rs"]
mod user_me_routes;
use user_me_routes::*;

pub(super) use self::user_me_routes::maybe_build_local_users_me_response;
