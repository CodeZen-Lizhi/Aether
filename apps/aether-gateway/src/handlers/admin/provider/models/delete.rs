use crate::handlers::admin::provider::shared::paths::admin_provider_model_route_parts;
use crate::handlers::admin::request::{AdminAppState, AdminRequestContext};
use crate::GatewayError;
use axum::{
    body::{Body, Bytes},
    http,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

pub(super) async fn maybe_handle(
    state: &AdminAppState<'_>,
    request_context: &AdminRequestContext<'_>,
    _request_body: Option<&Bytes>,
) -> Result<Option<Response<Body>>, GatewayError> {
    if request_context.route_family() == Some("provider_models_manage")
        && request_context.route_kind() == Some("delete_provider_model")
        && request_context.method() == http::Method::DELETE
        && request_context.path().contains("/models/")
    {
        let Some((provider_id, model_id)) =
            admin_provider_model_route_parts(request_context.path())
        else {
            return Ok(Some(
                (
                    http::StatusCode::NOT_FOUND,
                    Json(json!({ "detail": "Model 不存在" })),
                )
                    .into_response(),
            ));
        };
        let Some(existing) = state
            .get_admin_provider_model(&provider_id, &model_id)
            .await?
        else {
            return Ok(Some(
                (
                    http::StatusCode::NOT_FOUND,
                    Json(json!({ "detail": format!("Model {model_id} 不存在") })),
                )
                    .into_response(),
            ));
        };
        // 删除模型只解除密钥上的模型白名单关联，保留密钥和其它模型权限。
        let keys = state
            .list_provider_catalog_keys_by_provider_ids(&[provider_id.clone()])
            .await?;
        let model_names = [
            Some(existing.provider_model_name.as_str()),
            existing.global_model_name.as_deref(),
        ];
        let mut changed_keys = Vec::new();
        for mut key in keys {
            let Some(value) = key.allowed_models.as_ref() else {
                continue;
            };
            let Some(models) = value.as_array() else {
                continue;
            };
            let filtered = models
                .iter()
                .filter(|model| {
                    model.as_str().is_none_or(|name| {
                        !model_names.iter().flatten().any(|target| name == *target)
                    })
                })
                .cloned()
                .collect::<Vec<_>>();
            if filtered.len() != models.len() {
                key.allowed_models = Some(serde_json::Value::Array(filtered));
                changed_keys.push(key);
            }
        }
        if !changed_keys.is_empty() {
            state.update_provider_catalog_keys(&changed_keys).await?;
        }
        if !state
            .delete_admin_provider_model(&provider_id, &model_id)
            .await?
        {
            return Ok(Some(
                (
                    http::StatusCode::NOT_FOUND,
                    Json(json!({ "detail": format!("Model {model_id} 不存在") })),
                )
                    .into_response(),
            ));
        }
        return Ok(Some(
            Json(json!({
                "message": format!("Model '{}' deleted successfully", existing.provider_model_name),
            }))
            .into_response(),
        ));
    }

    Ok(None)
}
