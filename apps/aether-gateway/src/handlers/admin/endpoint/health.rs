use super::extractors::admin_recover_key_id;
use crate::handlers::admin::request::{AdminAppState, AdminRequestContext};
use crate::handlers::admin::shared::query_param_value;
use crate::GatewayError;
use axum::{
    body::Body,
    http,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

fn build_admin_endpoint_health_data_unavailable_response() -> Response<Body> {
    (
        http::StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({ "detail": "数据层不可用" })),
    )
        .into_response()
}

/// 个人版只保留「手动恢复熔断 Key」操作；健康监控查询页面已随裁剪移除。
pub(super) async fn maybe_build_local_admin_endpoints_health_response(
    state: &AdminAppState<'_>,
    request_context: &AdminRequestContext<'_>,
) -> Result<Option<Response<Body>>, GatewayError> {
    let decision = request_context.decision();
    let Some(decision) = decision else {
        return Ok(None);
    };
    if decision.route_family.as_deref() != Some("endpoints_health") {
        return Ok(None);
    }

    if decision.route_kind.as_deref() == Some("recover_key_health")
        && request_context
            .request_path
            .starts_with("/api/admin/endpoints/health/keys/")
    {
        if !state.has_provider_catalog_data_reader() || !state.has_provider_catalog_data_writer() {
            return Ok(Some(build_admin_endpoint_health_data_unavailable_response()));
        }
        let Some(key_id) = admin_recover_key_id(request_context.path()) else {
            return Ok(Some(
                (
                    http::StatusCode::NOT_FOUND,
                    Json(json!({ "detail": "Key 不存在" })),
                )
                    .into_response(),
            ));
        };
        let api_format = query_param_value(request_context.query_string(), "api_format");
        return Ok(Some(
            match state
                .recover_admin_key_health(&key_id, api_format.as_deref())
                .await
            {
                Some(payload) => Json(payload).into_response(),
                None => (
                    http::StatusCode::NOT_FOUND,
                    Json(json!({ "detail": format!("Key {key_id} 不存在") })),
                )
                    .into_response(),
            },
        ));
    }

    if decision.route_kind.as_deref() == Some("recover_all_keys_health")
        && request_context.path() == "/api/admin/endpoints/health/keys"
    {
        if !state.has_provider_catalog_data_reader() || !state.has_provider_catalog_data_writer() {
            return Ok(Some(build_admin_endpoint_health_data_unavailable_response()));
        }
        let Some(payload) = state.recover_all_admin_key_health().await else {
            return Ok(Some(build_admin_endpoint_health_data_unavailable_response()));
        };
        return Ok(Some(Json(payload).into_response()));
    }

    Ok(None)
}
