use crate::handlers::public::support::build_unhandled_public_support_response;
use axum::{body::Body, http, response::Response};

use super::{
    handle_auth_me, handle_users_me_delete_other_sessions, handle_users_me_delete_session,
    handle_users_me_detail_put, handle_users_me_password_patch, handle_users_me_preferences_get,
    handle_users_me_preferences_put, handle_users_me_sessions_get, handle_users_me_update_session,
    users_me_session_detail_path_matches, AppState, GatewayPublicRequestContext,
};

// 单用户版：users/me 只保留管理员自助能力（资料/密码/会话/偏好）。
// 其余历史用户侧端点（api-keys/usage/catalog/model-capabilities 等）已下线。
pub(crate) async fn maybe_build_local_users_me_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    request_body: Option<&axum::body::Bytes>,
) -> Option<Response<Body>> {
    let decision = request_context.control_decision.as_ref()?;
    if decision.route_family.as_deref() != Some("users_me") {
        return None;
    }

    match decision.route_kind.as_deref() {
        Some("detail") if request_context.request_path == "/api/users/me" => {
            Some(handle_auth_me(state, request_context, headers).await)
        }
        Some("update_detail") if request_context.request_path == "/api/users/me" => {
            Some(handle_users_me_detail_put(state, request_context, headers, request_body).await)
        }
        Some("password") if request_context.request_path == "/api/users/me/password" => Some(
            handle_users_me_password_patch(state, request_context, headers, request_body).await,
        ),
        Some("sessions") if request_context.request_path == "/api/users/me/sessions" => {
            Some(handle_users_me_sessions_get(state, request_context, headers).await)
        }
        Some("sessions_others_delete")
            if request_context.request_path == "/api/users/me/sessions/others" =>
        {
            Some(handle_users_me_delete_other_sessions(state, request_context, headers).await)
        }
        Some("session_delete")
            if users_me_session_detail_path_matches(&request_context.request_path) =>
        {
            Some(handle_users_me_delete_session(state, request_context, headers).await)
        }
        Some("session_update")
            if users_me_session_detail_path_matches(&request_context.request_path) =>
        {
            Some(
                handle_users_me_update_session(state, request_context, headers, request_body).await,
            )
        }
        Some("preferences") if request_context.request_path == "/api/users/me/preferences" => {
            Some(handle_users_me_preferences_get(state, request_context, headers).await)
        }
        Some("preferences_update")
            if request_context.request_path == "/api/users/me/preferences" =>
        {
            Some(
                handle_users_me_preferences_put(state, request_context, headers, request_body)
                    .await,
            )
        }
        _ => Some(build_unhandled_public_support_response(request_context)),
    }
}
