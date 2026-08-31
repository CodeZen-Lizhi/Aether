use super::super::internal;
use crate::admin_api;
use crate::audit::attach_admin_audit_event;
use crate::control::GatewayPublicRequestContext;
use crate::{AppState, GatewayError};
use axum::body::{Body, Bytes};
use axum::http::{self, Response};
use axum::response::IntoResponse;
use axum::Json;
use serde_json::json;
use tracing::warn;

pub(super) async fn maybe_build_local_internal_proxy_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    remote_addr: &std::net::SocketAddr,
    request_body: Option<&Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    internal::maybe_build_local_internal_proxy_response_impl(
        state,
        request_context,
        remote_addr,
        request_body,
    )
    .await
}

pub(super) async fn maybe_build_local_admin_proxy_response(
    state: &AppState,
    request_context: &GatewayPublicRequestContext,
    request_headers: &http::HeaderMap,
    request_body: Option<&Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    let Some(decision) = request_context.control_decision.as_ref() else {
        return Ok(None);
    };
    if decision.route_class.as_deref() != Some("admin_proxy") {
        return Ok(None);
    }
    if decision.admin_principal.is_none() {
        return Ok(None);
    }

    admin_api::maybe_build_local_admin_response(admin_api::AdminRouteRequest::new(
        state,
        request_context,
        request_headers,
        request_body,
    ))
    .await
}

