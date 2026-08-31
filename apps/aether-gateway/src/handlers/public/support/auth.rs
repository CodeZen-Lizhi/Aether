pub(super) use super::{
    build_unhandled_public_support_response, AppState, GatewayError, GatewayPublicRequestContext,
};
pub(super) use axum::{
    body::Body,
    http,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
pub(super) use serde_json::json;

#[path = "auth_helpers.rs"]
mod auth_helpers;
pub(crate) use auth_helpers::*;

#[path = "auth_session.rs"]
pub(super) mod auth_session;
use auth_session::*;

#[derive(Debug, Deserialize)]
struct AuthLoginRequest {
    email: String,
    password: String,
    #[serde(default = "default_auth_login_type")]
    auth_type: String,
}

fn default_auth_login_type() -> String {
    "local".to_string()
}

async fn handle_auth_login(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    request_body: Option<&axum::body::Bytes>,
) -> Response<Body> {
    let Some(request_body) = request_body else {
        return build_auth_error_response(http::StatusCode::BAD_REQUEST, "缺少登录请求体", false);
    };
    let payload = match serde_json::from_slice::<AuthLoginRequest>(request_body) {
        Ok(value) => value,
        Err(_) => {
            return build_auth_error_response(
                http::StatusCode::BAD_REQUEST,
                "无效的登录请求",
                false,
            )
        }
    };
    let identifier = normalize_auth_login_identifier(&payload.email);
    if identifier.is_empty() {
        return build_auth_error_response(
            http::StatusCode::BAD_REQUEST,
            "邮箱或用户名不能为空",
            false,
        );
    }
    if let Err(detail) = validate_auth_login_password(&payload.password) {
        return build_auth_error_response(http::StatusCode::BAD_REQUEST, detail, false);
    }
    let client_device_id = match extract_client_device_id(request_context, headers) {
        Ok(value) => value,
        Err(response) => return response,
    };
    let auth_type = payload.auth_type.trim().to_ascii_lowercase();
    let user = match auth_type.as_str() {
        "local" => {
            let user = match state.find_user_auth_by_identifier(&identifier).await {
                Ok(Some(user)) => user,
                Ok(None) => {
                    return build_auth_error_response(
                        http::StatusCode::UNAUTHORIZED,
                        "邮箱或密码错误",
                        false,
                    )
                }
                Err(err) => {
                    return build_auth_error_response(
                        http::StatusCode::INTERNAL_SERVER_ERROR,
                        format!("auth user lookup failed: {err:?}"),
                        false,
                    )
                }
            };
            if user.is_deleted
                || !user.is_active
                || !user.auth_source.eq_ignore_ascii_case("local")
                || user.password_hash.as_deref().is_none_or(str::is_empty)
            {
                return build_auth_error_response(
                    http::StatusCode::UNAUTHORIZED,
                    "邮箱或密码错误",
                    false,
                );
            }
            let password_hash = user
                .password_hash
                .as_deref()
                .expect("validated password hash should exist");
            let password_matches =
                bcrypt::verify(&payload.password, password_hash).unwrap_or(false);
            if !password_matches {
                return build_auth_error_response(
                    http::StatusCode::UNAUTHORIZED,
                    "邮箱或密码错误",
                    false,
                );
            }
            user
        }
        _ => {
            return build_auth_error_response(
                http::StatusCode::BAD_REQUEST,
                "不支持的认证类型",
                false,
            )
        }
    };

    build_auth_login_success_response(state, headers, client_device_id, user).await
}

pub(super) async fn maybe_build_local_auth_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    headers: &http::HeaderMap,
    cf_connecting_ip: Option<&str>,
    request_body: Option<&axum::body::Bytes>,
) -> Option<Response<Body>> {
    let decision = request_context.control_decision.as_ref()?;
    if decision.route_family.as_deref() != Some("auth") {
        return None;
    }

    match decision.route_kind.as_deref() {
        Some("login") if request_context.request_path == "/api/auth/login" => {
            Some(handle_auth_login(state, request_context, headers, request_body).await)
        }
        Some("me") if request_context.request_path == "/api/auth/me" => {
            Some(handle_auth_me(state, request_context, headers).await)
        }
        Some("refresh") if request_context.request_path == "/api/auth/refresh" => {
            Some(handle_auth_refresh(state, request_context, headers).await)
        }
        Some("logout") if request_context.request_path == "/api/auth/logout" => {
            Some(handle_auth_logout(state, request_context, headers).await)
        }
        _ => Some(build_unhandled_public_support_response(request_context)),
    }
}

#[cfg(test)]
mod tests {
    use super::{maybe_build_local_auth_response, AppState, GatewayPublicRequestContext};
    use crate::control::GatewayControlDecision;
    use axum::body::to_bytes;
    use axum::http::{HeaderMap, Method, StatusCode, Uri};

    fn request_context(method: Method, uri: &str, route_kind: &str) -> GatewayPublicRequestContext {
        GatewayPublicRequestContext::from_request_parts(
            "trace-auth-unhandled",
            &method,
            &uri.parse::<Uri>().expect("uri should parse"),
            &HeaderMap::new(),
            Some(GatewayControlDecision::synthetic(
                uri,
                Some("public_support".to_string()),
                Some("auth".to_string()),
                Some(route_kind.to_string()),
                Some("user:auth".to_string()),
            )),
        )
    }

    #[tokio::test]
    async fn auth_unhandled_route_returns_local_not_implemented_response() {
        let state = AppState::new().expect("gateway should build");
        let request_context = request_context(Method::POST, "/api/auth/login/history", "login");
        let response = maybe_build_local_auth_response(
            &state,
            &request_context,
            &HeaderMap::new(),
            None,
            None,
        )
        .await
        .expect("auth handler should return response");

        assert_eq!(response.status(), StatusCode::NOT_IMPLEMENTED);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body should read");
        let payload: serde_json::Value =
            serde_json::from_slice(&body).expect("json body should parse");
        assert_eq!(
            payload["detail"],
            "public support route not implemented in rust frontdoor"
        );
        assert_eq!(payload["route_family"], "auth");
        assert_eq!(payload["route_kind"], "login");
        assert_eq!(payload["request_path"], "/api/auth/login/history");
    }
}
