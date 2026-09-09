use axum::extract::State;
use axum::http::{header::CACHE_CONTROL, HeaderMap, HeaderValue, Method, StatusCode, Uri};

use super::{
    auth_session::build_auth_login_success_response, build_auth_error_response,
    extract_client_device_id, AppState, Body, GatewayPublicRequestContext, Response,
};

/// The host capability only permits this exchange. All management and proxy routes
/// retain their existing access-token, device-session, and API-key authentication.
pub(crate) async fn handle_desktop_session(
    State(state): State<AppState>,
    uri: Uri,
    mut headers: HeaderMap,
) -> Response<Body> {
    let Some(desktop) = state.desktop_session.as_ref() else {
        return build_auth_error_response(StatusCode::NOT_FOUND, "桌面会话入口未启用", false);
    };
    if !desktop.matches_origin(&headers) {
        return build_auth_error_response(StatusCode::FORBIDDEN, "桌面会话来源无效", false);
    }
    if !desktop.authorizes_capability(&headers) {
        return build_auth_error_response(StatusCode::UNAUTHORIZED, "桌面会话凭据无效", false);
    }
    // The shared login code needs device/user-agent metadata, never this capability.
    headers.remove("x-aether-desktop-session");
    let request_context = GatewayPublicRequestContext::from_request_parts(
        uuid::Uuid::new_v4().to_string(),
        &Method::POST,
        &uri,
        &headers,
        None,
    );
    let client_device_id = match extract_client_device_id(&request_context, &headers) {
        Ok(device_id) => device_id,
        Err(response) => return response,
    };
    // The user ID is fixed during startup, never supplied by the request. Re-read
    // the record so a disabled/deleted administrator cannot obtain a new session.
    let user = match state.find_user_auth_by_id(&desktop.user_id).await {
        Ok(Some(user)) if crate::state::is_usable_desktop_admin(&user) => user,
        Ok(_) => {
            return build_auth_error_response(
                StatusCode::UNAUTHORIZED,
                "本机管理员已失效，请检查本机数据后重启客户端",
                false,
            );
        }
        Err(_) => {
            return build_auth_error_response(
                StatusCode::SERVICE_UNAVAILABLE,
                "本机管理员数据暂时不可用，请重试",
                false,
            );
        }
    };

    let mut response =
        build_auth_login_success_response(&state, &headers, client_device_id, user).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}
