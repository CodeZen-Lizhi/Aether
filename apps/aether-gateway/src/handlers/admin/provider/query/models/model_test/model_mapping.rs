use crate::handlers::admin::request::AdminAppState;
use crate::GatewayError;
use aether_data_contracts::repository::candidate_selection::StoredProviderModelMapping;
use aether_data_contracts::repository::global_models::{
    AdminProviderModelListQuery, StoredAdminProviderModel,
};
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogEndpoint;
use serde_json::Value;
use std::collections::BTreeMap;

/// 模型测试中单个实际模型及其来源映射；映射为空表示未配置映射时的直连回退。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ProviderQueryTestEffectiveModel {
    /// 发给上游的实际模型名。
    pub name: String,
    /// 当前候选对应的配置映射名。
    pub mapping_name: Option<String>,
}

/// 按映射优先级解析模型测试要执行的映射。
///
/// 结果按 `priority` 升序、名称升序排列；未配置可用映射时回退请求模型。
/// 指定了映射但不存在或与当前端点不兼容时返回 `None`，由调用方转换为 400。
pub(super) async fn provider_query_resolve_test_effective_models(
    state: &AdminAppState<'_>,
    provider_id: &str,
    requested_model: &str,
    endpoint: &StoredProviderCatalogEndpoint,
    selected_mapping_names: Option<&[String]>,
) -> Result<Option<Vec<ProviderQueryTestEffectiveModel>>, GatewayError> {
    let models = state
        .list_admin_provider_models(&AdminProviderModelListQuery {
            provider_id: provider_id.to_string(),
            is_active: Some(true),
            offset: 0,
            limit: 1024,
        })
        .await
        .unwrap_or_default();

    let mut available_mappings = BTreeMap::<String, (String, i32)>::new();
    for model in models
        .into_iter()
        .filter(|model| model.is_available && model.is_active)
    {
        let mappings =
            provider_query_parse_provider_model_mappings(model.provider_model_mappings.as_ref())?;
        if !provider_query_admin_model_matches_requested_model(
            &model,
            mappings.as_deref(),
            requested_model,
        ) {
            continue;
        }

        let Some(model_mappings) = mappings else {
            continue;
        };
        for mapping in model_mappings {
            if !provider_query_model_mapping_matches_endpoint(&mapping, endpoint) {
                continue;
            }
            let normalized = mapping.name.trim().to_lowercase();
            if normalized.is_empty() {
                continue;
            }
            available_mappings
                .entry(normalized)
                .and_modify(|current| {
                    if mapping.priority < current.1 {
                        *current = (mapping.name.clone(), mapping.priority);
                    }
                })
                .or_insert((mapping.name, mapping.priority));
        }
    }

    let mut ordered_mappings = available_mappings.into_values().collect::<Vec<_>>();
    ordered_mappings.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| left.0.to_lowercase().cmp(&right.0.to_lowercase()))
    });

    let Some(selected_mapping_names) = selected_mapping_names.filter(|names| !names.is_empty())
    else {
        return Ok(Some(if ordered_mappings.is_empty() {
            vec![ProviderQueryTestEffectiveModel {
                name: requested_model.to_string(),
                mapping_name: None,
            }]
        } else {
            ordered_mappings
                .into_iter()
                .map(|(name, _)| ProviderQueryTestEffectiveModel {
                    mapping_name: Some(name.clone()),
                    name,
                })
                .collect()
        }));
    };

    let mut selected = Vec::<(String, i32)>::new();
    for selected_name in selected_mapping_names {
        let normalized = selected_name.trim().to_lowercase();
        if normalized.is_empty() {
            continue;
        }
        let Some((name, priority)) = ordered_mappings
            .iter()
            .find(|(name, _)| name.to_lowercase() == normalized)
        else {
            return Ok(None);
        };
        if selected
            .iter()
            .any(|(selected_name, _)| selected_name.eq_ignore_ascii_case(name))
        {
            continue;
        }
        selected.push((name.clone(), *priority));
    }
    selected.sort_by(|left, right| {
        left.1
            .cmp(&right.1)
            .then_with(|| left.0.to_lowercase().cmp(&right.0.to_lowercase()))
    });
    Ok(Some(
        selected
            .into_iter()
            .map(|(name, _)| ProviderQueryTestEffectiveModel {
                mapping_name: Some(name.clone()),
                name,
            })
            .collect(),
    ))
}

fn provider_query_model_mapping_matches_endpoint(
    mapping: &StoredProviderModelMapping,
    endpoint: &StoredProviderCatalogEndpoint,
) -> bool {
    let api_format_matches = mapping.api_formats.as_ref().is_none_or(|api_formats| {
        api_formats.is_empty()
            || api_formats.iter().any(|value| {
                crate::ai_serving::api_format_permission_covers(value, &endpoint.api_format)
            })
    });
    if !api_format_matches {
        return false;
    }

    mapping.endpoint_ids.as_ref().is_none_or(|endpoint_ids| {
        endpoint_ids.is_empty()
            || endpoint_ids
                .iter()
                .any(|endpoint_id| endpoint_id == &endpoint.id)
    })
}

fn provider_query_admin_model_matches_requested_model(
    model: &StoredAdminProviderModel,
    mappings: Option<&[StoredProviderModelMapping]>,
    requested_model: &str,
) -> bool {
    model
        .global_model_name
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case(requested_model))
        || model
            .provider_model_name
            .eq_ignore_ascii_case(requested_model)
        || mappings.is_some_and(|mappings| {
            mappings
                .iter()
                .any(|mapping| mapping.name.eq_ignore_ascii_case(requested_model))
        })
}

fn provider_query_parse_provider_model_mappings(
    value: Option<&Value>,
) -> Result<Option<Vec<StoredProviderModelMapping>>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    provider_query_parse_provider_model_mappings_value(value)
}

fn provider_query_parse_provider_model_mappings_value(
    value: &Value,
) -> Result<Option<Vec<StoredProviderModelMapping>>, GatewayError> {
    match value {
        Value::Null => Ok(None),
        Value::Array(items) => provider_query_parse_provider_model_mappings_array(items),
        Value::Object(object) => provider_query_parse_provider_model_mapping_object(object)
            .map(|mapping| Some(vec![mapping])),
        Value::String(raw) => provider_query_parse_embedded_provider_model_mappings(raw),
        _ => Err(GatewayError::Internal(
            "models.provider_model_mappings is not a JSON array".to_string(),
        )),
    }
}

fn provider_query_parse_embedded_provider_model_mappings(
    raw: &str,
) -> Result<Option<Vec<StoredProviderModelMapping>>, GatewayError> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
        return Ok(None);
    }

    if let Ok(decoded) = serde_json::from_str::<Value>(raw) {
        return provider_query_parse_provider_model_mappings_value(&decoded);
    }

    Ok(Some(vec![StoredProviderModelMapping {
        name: raw.to_string(),
        priority: 1,
        api_formats: None,
        endpoint_ids: None,
        operations: None,
    }]))
}

fn provider_query_parse_provider_model_mappings_array(
    items: &[Value],
) -> Result<Option<Vec<StoredProviderModelMapping>>, GatewayError> {
    let mut mappings = Vec::with_capacity(items.len());
    for item in items {
        match item {
            Value::Object(object) => {
                if let Some(mapping) =
                    provider_query_parse_provider_model_mapping_object_lenient(object)?
                {
                    mappings.push(mapping);
                }
            }
            Value::String(raw) => {
                let raw = raw.trim();
                if !raw.is_empty() {
                    mappings.push(StoredProviderModelMapping {
                        name: raw.to_string(),
                        priority: 1,
                        api_formats: None,
                        endpoint_ids: None,
                        operations: None,
                    });
                }
            }
            Value::Null => {}
            _ => {}
        }
    }

    if mappings.is_empty() {
        Ok(None)
    } else {
        Ok(Some(mappings))
    }
}

fn provider_query_parse_provider_model_mapping_object(
    object: &serde_json::Map<String, Value>,
) -> Result<StoredProviderModelMapping, GatewayError> {
    provider_query_parse_provider_model_mapping_object_lenient(object)?.ok_or_else(|| {
        GatewayError::Internal(
            "models.provider_model_mappings item is missing a valid name".to_string(),
        )
    })
}

fn provider_query_parse_provider_model_mapping_object_lenient(
    object: &serde_json::Map<String, Value>,
) -> Result<Option<StoredProviderModelMapping>, GatewayError> {
    let Some(name) = object
        .get("name")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };

    let priority = object
        .get("priority")
        .and_then(Value::as_i64)
        .unwrap_or(1)
        .max(1);
    let api_formats = provider_query_parse_mapping_string_list(
        object.get("api_formats"),
        "models.provider_model_mappings.api_formats",
    )?
    .map(|formats| {
        formats
            .into_iter()
            .map(|value| crate::ai_serving::normalize_api_format_alias(&value))
            .collect()
    });
    let endpoint_ids = provider_query_parse_mapping_string_list(
        object.get("endpoint_ids"),
        "models.provider_model_mappings.endpoint_ids",
    )?;
    let operations = provider_query_parse_mapping_string_list(
        object.get("operations"),
        "models.provider_model_mappings.operations",
    )?
    .and_then(provider_query_normalize_request_operations);

    Ok(Some(StoredProviderModelMapping {
        name: name.to_string(),
        priority: i32::try_from(priority).map_err(|_| {
            GatewayError::Internal(format!(
                "invalid models.provider_model_mappings.priority: {priority}"
            ))
        })?,
        api_formats,
        endpoint_ids,
        operations,
    }))
}

fn provider_query_normalize_request_operations(values: Vec<String>) -> Option<Vec<String>> {
    let operations = values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    (!operations.is_empty()).then_some(operations)
}

fn provider_query_parse_mapping_string_list(
    value: Option<&Value>,
    field_name: &str,
) -> Result<Option<Vec<String>>, GatewayError> {
    let Some(value) = value else {
        return Ok(None);
    };
    provider_query_parse_mapping_string_list_value(value, field_name)
}

fn provider_query_parse_mapping_string_list_value(
    value: &Value,
    field_name: &str,
) -> Result<Option<Vec<String>>, GatewayError> {
    match value {
        Value::Null => Ok(None),
        Value::Array(items) => {
            provider_query_parse_mapping_string_list_array(items, field_name).map(Some)
        }
        Value::String(raw) => provider_query_parse_embedded_mapping_string_list(raw, field_name),
        _ => Err(GatewayError::Internal(format!(
            "{field_name} is not a JSON array"
        ))),
    }
}

fn provider_query_parse_embedded_mapping_string_list(
    raw: &str,
    field_name: &str,
) -> Result<Option<Vec<String>>, GatewayError> {
    let raw = raw.trim();
    if raw.is_empty() || raw.eq_ignore_ascii_case("null") {
        return Ok(None);
    }

    if let Ok(decoded) = serde_json::from_str::<Value>(raw) {
        return provider_query_parse_mapping_string_list_value(&decoded, field_name);
    }

    Ok(Some(vec![raw.to_string()]))
}

fn provider_query_parse_mapping_string_list_array(
    items: &[Value],
    field_name: &str,
) -> Result<Vec<String>, GatewayError> {
    let mut parsed = Vec::with_capacity(items.len());
    for item in items {
        let Some(item) = item.as_str() else {
            return Err(GatewayError::Internal(format!(
                "{field_name} contains a non-string item"
            )));
        };
        let item = item.trim();
        if !item.is_empty() {
            parsed.push(item.to_string());
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_endpoint(api_format: &str) -> StoredProviderCatalogEndpoint {
        StoredProviderCatalogEndpoint::new(
            "endpoint-1".to_string(),
            "provider-1".to_string(),
            api_format.to_string(),
            None,
            None,
            true,
        )
        .expect("endpoint should build")
    }

    fn sample_mapping(api_format: &str) -> StoredProviderModelMapping {
        StoredProviderModelMapping {
            name: "gpt-5.6-luna".to_string(),
            priority: 1,
            api_formats: Some(vec![api_format.to_string()]),
            endpoint_ids: None,
            operations: None,
        }
    }

    #[test]
    fn responses_mapping_scope_covers_search_in_one_direction() {
        assert!(provider_query_model_mapping_matches_endpoint(
            &sample_mapping("openai:responses"),
            &sample_endpoint("openai:search"),
        ));
        assert!(!provider_query_model_mapping_matches_endpoint(
            &sample_mapping("openai:search"),
            &sample_endpoint("openai:responses"),
        ));
    }

    #[test]
    fn empty_mapping_scopes_match_every_endpoint() {
        let mut mapping = sample_mapping("openai:responses");
        mapping.api_formats = Some(Vec::new());
        mapping.endpoint_ids = Some(Vec::new());

        assert!(provider_query_model_mapping_matches_endpoint(
            &mapping,
            &sample_endpoint("openai:chat"),
        ));
    }
}
