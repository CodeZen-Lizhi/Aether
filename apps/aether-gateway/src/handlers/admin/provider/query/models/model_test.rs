use super::super::payload::{
    provider_query_extract_api_key_ids, provider_query_extract_force_refresh,
    provider_query_extract_model, provider_query_extract_provider_id,
    provider_query_extract_request_id,
};
use super::super::response::{
    build_admin_provider_query_bad_request_response, build_admin_provider_query_not_found_response,
    ADMIN_PROVIDER_QUERY_API_KEY_NOT_FOUND_DETAIL, ADMIN_PROVIDER_QUERY_MODEL_REQUIRED_DETAIL,
    ADMIN_PROVIDER_QUERY_NO_ACTIVE_API_KEY_DETAIL, ADMIN_PROVIDER_QUERY_NO_LOCAL_MODELS_DETAIL,
    ADMIN_PROVIDER_QUERY_PROVIDER_ID_REQUIRED_DETAIL,
    ADMIN_PROVIDER_QUERY_PROVIDER_NOT_FOUND_DETAIL,
};
use super::{provider_query_key_display_name, provider_query_provider_payload};
use crate::ai_serving::{
    maybe_build_sync_finalize_outcome, GatewayControlDecision,
    ANTIGRAVITY_V1INTERNAL_ENVELOPE_NAME, GEMINI_CHAT_SYNC_FINALIZE_REPORT_KIND,
    OPENAI_CHAT_SYNC_FINALIZE_REPORT_KIND, OPENAI_IMAGE_SYNC_FINALIZE_REPORT_KIND,
};
use crate::clock::current_unix_ms;
use crate::execution_runtime;
use crate::handlers::admin::request::{AdminAppState, AdminGatewayProviderTransportSnapshot};
use crate::handlers::shared::{
    parse_catalog_auth_config_json, provider_key_health_summary,
    provider_key_status_snapshot_payload,
};
use crate::model_fetch::ModelFetchRuntimeState;
use crate::provider_key_auth::provider_key_auth_semantics;
use crate::usage::GatewaySyncReportRequest;
use crate::{AppState, GatewayError};
use aether_contracts::{ExecutionPlan, RequestBody};
use aether_data_contracts::repository::candidate_selection::{
    StoredMinimalCandidateSelectionRow, StoredProviderModelMapping,
};
use aether_data_contracts::repository::candidates::{
    RequestCandidateStatus, UpsertRequestCandidateRecord,
};
use aether_data_contracts::repository::global_models::{
    AdminProviderModelListQuery, StoredAdminProviderModel,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogEndpoint, StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use aether_model_fetch::{
    aggregate_models_for_cache, fetch_models_from_transports, json_string_list,
    selected_models_fetch_endpoints,
};
use aether_scheduler_core::provider_key_circuit_payload_is_active_open_at;
use axum::{
    body::{to_bytes, Body},
    http::{self, HeaderMap, HeaderName, HeaderValue},
    response::{IntoResponse, Response},
    Json,
};
use base64::Engine as _;
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use tracing::{debug, warn};
use uuid::Uuid;

mod adapter;
mod capabilities;
mod model_mapping;
mod summary;

use self::adapter::{
    provider_query_antigravity_test_unsupported_reason,
    provider_query_antigravity_unsupported_reason,
    provider_query_default_antigravity_endpoint_test_body,
    provider_query_grok_test_unsupported_reason, provider_query_model_test_endpoint_priority,
    provider_query_normalize_api_format_alias, provider_query_standard_test_client_api_format,
    provider_query_standard_test_unsupported_reason,
    provider_query_test_adapter_for_provider_api_format,
    provider_query_transport_supports_model_test_execution,
    provider_query_unsupported_test_api_format_message, ProviderQueryTestAdapter,
};
use self::capabilities::{
    provider_query_openai_image_normalize_failure_message,
    provider_query_openai_image_normalize_options,
};
use self::model_mapping::{
    provider_query_resolve_explicit_mapped_effective_model,
    provider_query_resolve_global_effective_model,
};
use self::summary::{
    provider_query_candidate_summary_payload, provider_query_test_attempt_payload,
};

pub(crate) const ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_MESSAGE: &str =
    "Rust local provider-query model test is not configured";
pub(crate) const ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_FAILOVER_MESSAGE: &str =
    "Rust local provider-query failover simulation is not configured";
const ADMIN_PROVIDER_QUERY_NO_ACTIVE_ENDPOINT_DETAIL: &str =
    "No active endpoints found for this provider";
const ADMIN_PROVIDER_QUERY_NO_MODELS_FROM_ENDPOINT_DETAIL: &str =
    "No models returned from any endpoint";
const ADMIN_PROVIDER_QUERY_NO_MODELS_FROM_KEY_DETAIL: &str = "No models returned from any key";
const ADMIN_PROVIDER_QUERY_NO_ACTIVE_TEST_CANDIDATE_DETAIL: &str =
    "No active endpoint or API key found";
const ADMIN_PROVIDER_QUERY_INVALID_MAPPED_MODEL_DETAIL: &str =
    "mapped_model_name is not valid for the selected model and endpoint";
const PROVIDER_QUERY_KEY_MODEL_NOT_ALLOWED_SKIP_REASON: &str = "key_model_not_allowed";
const ANTIGRAVITY_PROVIDER_CACHE_KEY_PREFIX: &str = "upstream_models_provider:";
const DEFAULT_PROVIDER_QUERY_TEST_MESSAGE: &str = "Hello! This is a test message.";
struct ProviderQueryTestCandidate {
    endpoint: StoredProviderCatalogEndpoint,
    key: StoredProviderCatalogKey,
    effective_model: String,
    scheduler_skip_reason: Option<String>,
}

#[derive(Debug, Clone)]
struct ProviderQueryTestAttempt {
    candidate_index: usize,
    endpoint_api_format: String,
    endpoint_base_url: String,
    key_name: String,
    key_id: String,
    auth_type: String,
    effective_model: String,
    status: &'static str,
    skip_reason: Option<String>,
    error_message: Option<String>,
    status_code: Option<u16>,
    latency_ms: Option<u64>,
    request_url: Option<String>,
    request_headers: Option<BTreeMap<String, String>>,
    request_body: Option<Value>,
    response_headers: Option<BTreeMap<String, String>>,
    response_body: Option<Value>,
}

#[derive(Debug, Clone)]
struct ProviderQueryExecutionOutcome {
    status: &'static str,
    skip_reason: Option<String>,
    error_message: Option<String>,
    status_code: Option<u16>,
    latency_ms: Option<u64>,
    request_url: String,
    request_headers: BTreeMap<String, String>,
    request_body: Value,
    response_headers: BTreeMap<String, String>,
    response_body: Option<Value>,
}

#[derive(Default)]
struct ProviderQueryTestTraceUpdate<'a> {
    skip_reason: Option<&'a str>,
    error_message: Option<&'a str>,
    status_code: Option<u16>,
    latency_ms: Option<u64>,
    started_at_unix_ms: Option<u64>,
    finished_at_unix_ms: Option<u64>,
}

fn provider_query_test_candidate_trace_id(trace_id: &str, candidate_index: usize) -> String {
    Uuid::new_v5(
        &Uuid::NAMESPACE_URL,
        format!("aether:provider-query:model-test:{trace_id}:{candidate_index}").as_bytes(),
    )
    .to_string()
}

fn provider_query_test_candidate_trace_index(candidate_index: usize) -> Option<u32> {
    match u32::try_from(candidate_index) {
        Ok(value) => Some(value),
        Err(_) => {
            warn!(
                event_name = "provider_query_model_test_trace_index_overflow",
                log_type = "event",
                candidate_index,
                "gateway skipped admin model-test candidate trace because candidate index exceeds u32"
            );
            None
        }
    }
}

fn provider_query_test_candidate_trace_extra_data(
    provider: &StoredProviderCatalogProvider,
    candidate: &ProviderQueryTestCandidate,
) -> Value {
    json!({
        "admin_model_test": {
            "provider_type": provider.provider_type,
            "endpoint_api_format": candidate.endpoint.api_format,
            "endpoint_base_url": candidate.endpoint.base_url,
            "effective_model": candidate.effective_model,
        }
    })
}

async fn provider_query_persist_test_candidate_trace(
    state: &AppState,
    trace_id: &str,
    provider: &StoredProviderCatalogProvider,
    candidate: &ProviderQueryTestCandidate,
    candidate_index: usize,
    status: RequestCandidateStatus,
    update: ProviderQueryTestTraceUpdate<'_>,
) {
    if !state.has_request_candidate_data_writer() {
        return;
    }
    let Some(candidate_index_u32) = provider_query_test_candidate_trace_index(candidate_index)
    else {
        return;
    };
    let candidate_id = provider_query_test_candidate_trace_id(trace_id, candidate_index);
    let status_label = format!("{status:?}");
    let record = UpsertRequestCandidateRecord {
        id: candidate_id.clone(),
        request_id: trace_id.to_string(),
        user_id: None,
        api_key_id: None,
        username: None,
        api_key_name: None,
        candidate_index: candidate_index_u32,
        retry_index: 0,
        provider_id: Some(provider.id.clone()),
        endpoint_id: Some(candidate.endpoint.id.clone()),
        key_id: Some(candidate.key.id.clone()),
        status,
        skip_reason: update.skip_reason.map(ToOwned::to_owned),
        is_cached: Some(false),
        status_code: update.status_code,
        error_type: None,
        error_message: update.error_message.map(ToOwned::to_owned),
        latency_ms: update.latency_ms,
        concurrent_requests: None,
        extra_data: Some(provider_query_test_candidate_trace_extra_data(
            provider, candidate,
        )),
        required_capabilities: None,
        created_at_unix_ms: Some(current_unix_ms()),
        started_at_unix_ms: update.started_at_unix_ms,
        finished_at_unix_ms: update.finished_at_unix_ms,
    };

    match state.upsert_request_candidate(record).await {
        Ok(Some(stored)) => {
            debug!(
                event_name = "provider_query_model_test_trace_persisted",
                log_type = "event",
                request_id = %trace_id,
                candidate_id = %stored.id,
                candidate_index,
                key_id = %candidate.key.id,
                endpoint_id = %candidate.endpoint.id,
                status = %status_label,
                "gateway persisted admin model-test candidate trace"
            );
        }
        Ok(None) => {
            warn!(
                event_name = "provider_query_model_test_trace_writer_unavailable",
                log_type = "event",
                request_id = %trace_id,
                candidate_id = %candidate_id,
                candidate_index,
                key_id = %candidate.key.id,
                endpoint_id = %candidate.endpoint.id,
                status = %status_label,
                "gateway skipped admin model-test candidate trace because writer is unavailable"
            );
        }
        Err(error) => {
            warn!(
                event_name = "provider_query_model_test_trace_persist_failed",
                log_type = "event",
                request_id = %trace_id,
                candidate_id = %candidate_id,
                candidate_index,
                key_id = %candidate.key.id,
                endpoint_id = %candidate.endpoint.id,
                status = %status_label,
                error = ?error,
                "gateway failed to persist admin model-test candidate trace"
            );
        }
    }
}

async fn provider_query_seed_test_candidate_traces(
    state: &AppState,
    trace_id: &str,
    provider: &StoredProviderCatalogProvider,
    candidates: &[ProviderQueryTestCandidate],
) {
    for (candidate_index, candidate) in candidates.iter().enumerate() {
        provider_query_persist_test_candidate_trace(
            state,
            trace_id,
            provider,
            candidate,
            candidate_index,
            RequestCandidateStatus::Available,
            ProviderQueryTestTraceUpdate::default(),
        )
        .await;
    }
}

async fn provider_query_mark_unused_test_candidate_traces(
    state: &AppState,
    trace_id: &str,
    provider: &StoredProviderCatalogProvider,
    candidates: &[ProviderQueryTestCandidate],
    first_unused_index: usize,
) {
    let finished_at_unix_ms = current_unix_ms();
    for (candidate_index, candidate) in candidates.iter().enumerate().skip(first_unused_index) {
        provider_query_persist_test_candidate_trace(
            state,
            trace_id,
            provider,
            candidate,
            candidate_index,
            RequestCandidateStatus::Unused,
            ProviderQueryTestTraceUpdate {
                finished_at_unix_ms: Some(finished_at_unix_ms),
                ..ProviderQueryTestTraceUpdate::default()
            },
        )
        .await;
    }
}

async fn provider_query_mark_pending_test_candidate_trace(
    state: &AppState,
    trace_id: &str,
    provider: &StoredProviderCatalogProvider,
    candidate: &ProviderQueryTestCandidate,
    candidate_index: usize,
) {
    provider_query_persist_test_candidate_trace(
        state,
        trace_id,
        provider,
        candidate,
        candidate_index,
        RequestCandidateStatus::Pending,
        ProviderQueryTestTraceUpdate {
            started_at_unix_ms: Some(current_unix_ms()),
            ..ProviderQueryTestTraceUpdate::default()
        },
    )
    .await;
}

async fn provider_query_finish_test_candidate_trace(
    state: &AppState,
    trace_id: &str,
    provider: &StoredProviderCatalogProvider,
    candidate: &ProviderQueryTestCandidate,
    candidate_index: usize,
    execution: &ProviderQueryExecutionOutcome,
) {
    let status = match execution.status {
        "success" => RequestCandidateStatus::Success,
        "skipped" => RequestCandidateStatus::Skipped,
        _ => RequestCandidateStatus::Failed,
    };
    provider_query_persist_test_candidate_trace(
        state,
        trace_id,
        provider,
        candidate,
        candidate_index,
        status,
        ProviderQueryTestTraceUpdate {
            skip_reason: execution.skip_reason.as_deref(),
            error_message: execution.error_message.as_deref(),
            status_code: execution.status_code,
            latency_ms: execution.latency_ms,
            finished_at_unix_ms: Some(current_unix_ms()),
            ..ProviderQueryTestTraceUpdate::default()
        },
    )
    .await;
}

fn provider_query_skipped_execution_outcome(
    request_body: Value,
    skip_reason: impl Into<String>,
) -> ProviderQueryExecutionOutcome {
    ProviderQueryExecutionOutcome {
        status: "skipped",
        skip_reason: Some(skip_reason.into()),
        error_message: None,
        status_code: None,
        latency_ms: None,
        request_url: String::new(),
        request_headers: BTreeMap::new(),
        request_body,
        response_headers: BTreeMap::new(),
        response_body: None,
    }
}

fn provider_query_default_local_test_error(route_path: &str) -> &'static str {
    if route_path.ends_with("/test-model") {
        ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_MESSAGE
    } else {
        ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_FAILOVER_MESSAGE
    }
}

fn provider_query_test_mode(payload: &Value) -> &str {
    payload
        .get("mode")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("global")
}

fn provider_query_should_apply_model_mapping(payload: &Value) -> bool {
    payload
        .get("apply_model_mapping")
        .and_then(Value::as_bool)
        .unwrap_or(true)
}

fn provider_query_extract_mapped_model_name(payload: &Value) -> Option<String> {
    payload
        .get("mapped_model_name")
        .or_else(|| payload.get("mapped_model"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn provider_query_extract_endpoint_id(payload: &Value) -> Option<String> {
    payload
        .get("endpoint_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn provider_query_extract_api_format(payload: &Value) -> Option<String> {
    payload
        .get("api_format")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn provider_query_extract_message(payload: &Value) -> Option<String> {
    payload
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn provider_query_extract_request_body(payload: &Value) -> Option<Value> {
    payload
        .get("request_body")
        .filter(|value| value.is_object())
        .cloned()
}

fn provider_query_extract_request_headers(payload: &Value) -> HeaderMap {
    let mut headers = HeaderMap::new();
    let Some(values) = payload.get("request_headers").and_then(Value::as_object) else {
        return headers;
    };
    for (key, value) in values {
        let key = key.trim();
        if key.is_empty() {
            continue;
        }
        let Some(value) = (match value {
            Value::String(value) => Some(value.trim().to_string()),
            Value::Bool(value) => Some(value.to_string()),
            Value::Number(value) => Some(value.to_string()),
            other => serde_json::to_string(other).ok(),
        }) else {
            continue;
        };
        if value.is_empty() {
            continue;
        }
        let Ok(name) = HeaderName::from_bytes(key.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(&value) else {
            continue;
        };
        headers.insert(name, value);
    }
    headers
}

fn provider_query_build_test_request_body(payload: &Value, model: &str) -> Value {
    provider_query_build_test_request_body_with_model_policy(payload, model, false)
}

fn provider_query_build_test_request_body_for_route(
    payload: &Value,
    model: &str,
    route_path: &str,
) -> Value {
    let override_custom_model = route_path.ends_with("/test-model-failover")
        || provider_query_extract_mapped_model_name(payload).is_some();
    provider_query_build_test_request_body_with_model_policy(payload, model, override_custom_model)
}

fn provider_query_build_test_request_body_for_api_format(
    payload: &Value,
    model: &str,
    route_path: &str,
    client_api_format: &str,
) -> Value {
    provider_query_build_test_request_body_for_api_format_with_search_session(
        payload,
        model,
        route_path,
        client_api_format,
        None,
    )
}

fn provider_query_build_test_request_body_for_api_format_with_search_session(
    payload: &Value,
    model: &str,
    route_path: &str,
    client_api_format: &str,
    search_session_id: Option<&str>,
) -> Value {
    let client_api_format = provider_query_normalize_api_format_alias(client_api_format);
    let override_custom_model = route_path.ends_with("/test-model-failover")
        || provider_query_extract_mapped_model_name(payload).is_some();
    if let Some(mut body) = provider_query_extract_request_body(payload) {
        let has_conversation = provider_query_request_body_has_conversation_for_api_format(
            &body,
            client_api_format.as_str(),
        );
        if let Some(object) = body.as_object_mut() {
            if override_custom_model {
                object.insert("model".to_string(), Value::String(model.to_string()));
            } else {
                object
                    .entry("model".to_string())
                    .or_insert_with(|| Value::String(model.to_string()));
            }
            if !has_conversation {
                provider_query_insert_default_test_conversation(
                    object,
                    client_api_format.as_str(),
                    payload,
                );
            } else if matches!(
                client_api_format.as_str(),
                "openai:responses" | "openai:responses:compact" | "openai:search"
            ) && !value_has_non_empty_text(object.get("input"))
            {
                if let Some(prompt) = object
                    .remove("prompt")
                    .filter(|value| value_has_non_empty_text(Some(value)))
                {
                    object.insert("input".to_string(), prompt);
                }
            }
            if matches!(
                client_api_format.as_str(),
                "openai:responses" | "openai:responses:compact" | "openai:search"
            ) && value_has_non_empty_text(object.get("input"))
            {
                object.remove("prompt");
            }
            if client_api_format == "openai:responses:compact"
                && value_has_non_empty_text(object.get("input"))
            {
                object.remove("messages");
            }
            if client_api_format == "openai:search" {
                provider_query_ensure_search_test_fields(object, payload, search_session_id);
            }
        }
        return body;
    }

    let message = provider_query_extract_message(payload)
        .unwrap_or_else(|| DEFAULT_PROVIDER_QUERY_TEST_MESSAGE.to_string());
    match client_api_format.as_str() {
        "openai:embedding" => json!({
            "model": model,
            "input": message,
        }),
        "openai:rerank" => json!({
            "model": model,
            "query": message,
            "documents": [
                "apple",
                "banana",
                "fruit",
                "vegetable"
            ],
            "return_documents": true,
            "top_n": 4,
        }),
        "openai:responses" | "openai:responses:compact" => json!({
            "model": model,
            "input": message,
            "max_output_tokens": 30,
            "stream": true,
        }),
        "openai:search" => json!({
            "id": provider_query_search_test_session_id(search_session_id),
            "model": model,
            "input": message,
            "commands": {
                "search_query": [{"q": message}]
            },
            "max_output_tokens": 256,
        }),
        "claude:messages" => json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": message
            }],
            "max_tokens": 30,
            "stream": true,
        }),
        _ => json!({
            "model": model,
            "messages": [{
                "role": "user",
                "content": message
            }],
            "max_tokens": 30,
            "stream": true,
        }),
    }
}

fn provider_query_build_grok_test_request_body_for_api_format(
    payload: &Value,
    model: &str,
    route_path: &str,
    client_api_format: &str,
) -> Value {
    provider_query_build_test_request_body_for_api_format(
        payload,
        model,
        route_path,
        client_api_format,
    )
}

fn provider_query_insert_default_test_conversation(
    object: &mut Map<String, Value>,
    client_api_format: &str,
    payload: &Value,
) {
    let message = provider_query_extract_message(payload)
        .unwrap_or_else(|| DEFAULT_PROVIDER_QUERY_TEST_MESSAGE.to_string());
    match client_api_format {
        "openai:embedding" => {
            object.insert("input".to_string(), Value::String(message));
        }
        "openai:rerank" => {
            object.insert("query".to_string(), Value::String(message));
            object
                .entry("documents".to_string())
                .or_insert_with(|| json!(["apple", "banana", "fruit", "vegetable"]));
            object
                .entry("return_documents".to_string())
                .or_insert(Value::Bool(true));
            object
                .entry("top_n".to_string())
                .or_insert_with(|| Value::from(4_u64));
        }
        "openai:responses" | "openai:responses:compact" | "openai:search" => {
            object.insert("input".to_string(), Value::String(message));
        }
        "claude:messages" => {
            object.insert(
                "messages".to_string(),
                json!([{ "role": "user", "content": message }]),
            );
        }
        _ => {
            object.insert(
                "messages".to_string(),
                json!([{ "role": "user", "content": message }]),
            );
        }
    }
}

fn provider_query_search_test_session_id(search_session_id: Option<&str>) -> String {
    search_session_id
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("aether-model-test-{value}"))
        .unwrap_or_else(|| format!("aether-model-test-{}", Uuid::new_v4().simple()))
}

fn provider_query_ensure_search_test_fields(
    object: &mut Map<String, Value>,
    payload: &Value,
    search_session_id: Option<&str>,
) {
    let query = object
        .get("input")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| provider_query_extract_message(payload))
        .unwrap_or_else(|| DEFAULT_PROVIDER_QUERY_TEST_MESSAGE.to_string());
    let has_session_id = object
        .get("id")
        .and_then(Value::as_str)
        .is_some_and(|value| !value.trim().is_empty());
    if !has_session_id {
        object.insert(
            "id".to_string(),
            provider_query_search_test_session_id(search_session_id).into(),
        );
    }
    object
        .entry("input".to_string())
        .or_insert_with(|| Value::String(query.clone()));
    object
        .entry("commands".to_string())
        .or_insert_with(|| json!({"search_query": [{"q": query}]}));
    object
        .entry("max_output_tokens".to_string())
        .or_insert_with(|| Value::from(256_u64));
}


fn provider_query_build_test_request_body_with_model_policy(
    payload: &Value,
    model: &str,
    override_custom_model: bool,
) -> Value {
    if let Some(mut body) = provider_query_extract_request_body(payload) {
        let has_conversation = provider_query_request_body_has_conversation(&body);
        if let Some(object) = body.as_object_mut() {
            if override_custom_model {
                object.insert("model".to_string(), Value::String(model.to_string()));
            } else {
                object
                    .entry("model".to_string())
                    .or_insert_with(|| Value::String(model.to_string()));
            }
            if !has_conversation {
                object.insert(
                    "messages".to_string(),
                    json!([{
                        "role": "user",
                        "content": provider_query_extract_message(payload)
                            .unwrap_or_else(|| DEFAULT_PROVIDER_QUERY_TEST_MESSAGE.to_string())
                    }]),
                );
            }
        }
        return body;
    }

    json!({
        "model": model,
        "messages": [{
            "role": "user",
            "content": provider_query_extract_message(payload)
                .unwrap_or_else(|| DEFAULT_PROVIDER_QUERY_TEST_MESSAGE.to_string())
        }],
        "stream": true,
    })
}

fn provider_query_request_body_has_conversation(body: &Value) -> bool {
    body.get("messages")
        .and_then(Value::as_array)
        .map(|messages| {
            messages
                .iter()
                .any(|message| value_has_non_empty_text(message.get("content")))
        })
        .unwrap_or(false)
        || value_has_non_empty_text(body.get("input"))
        || value_has_non_empty_text(body.get("prompt"))
        || value_has_non_empty_text(body.get("query"))
        || value_has_non_empty_text(body.get("system"))
}

fn provider_query_request_body_has_conversation_for_api_format(
    body: &Value,
    client_api_format: &str,
) -> bool {
    match provider_query_normalize_api_format_alias(client_api_format).as_str() {
        "openai:responses" | "openai:responses:compact" | "openai:search" => {
            value_has_non_empty_text(body.get("input"))
                || value_has_non_empty_text(body.get("prompt"))
        }
        "claude:messages" => {
            body.get("messages")
                .and_then(Value::as_array)
                .map(|messages| {
                    messages
                        .iter()
                        .any(|message| value_has_non_empty_text(message.get("content")))
                })
                .unwrap_or(false)
                || value_has_non_empty_text(body.get("system"))
        }
        _ => provider_query_request_body_has_conversation(body),
    }
}

fn provider_query_request_body_is_openai_responses_shape(body: &Value) -> bool {
    let Some(object) = body.as_object() else {
        return false;
    };
    [
        "input",
        "tools",
        "tool_choice",
        "instructions",
        "previous_response_id",
    ]
    .iter()
    .any(|key| object.contains_key(*key))
}

fn value_has_non_empty_text(value: Option<&Value>) -> bool {
    match value {
        Some(Value::String(value)) => !value.trim().is_empty(),
        Some(Value::Array(values)) => values
            .iter()
            .any(|value| value_has_non_empty_text(Some(value))),
        Some(Value::Object(values)) => values
            .values()
            .any(|value| value_has_non_empty_text(Some(value))),
        _ => false,
    }
}

fn provider_query_request_body_model<'a>(request_body: &'a Value, fallback: &'a str) -> &'a str {
    request_body
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
}

fn provider_query_resolve_standard_test_upstream_is_stream(
    endpoint_config: Option<&Value>,
    provider_type: &str,
    provider_api_format: &str,
) -> bool {
    crate::ai_serving::resolve_upstream_is_stream_for_provider(
        endpoint_config,
        provider_type,
        provider_api_format,
        false,
        false,
    )
}

fn provider_query_request_requires_body_stream_field(
    request_body: &Value,
    endpoint_config: Option<&Value>,
) -> bool {
    crate::ai_serving::endpoint_config_forces_upstream_stream_policy(endpoint_config)
        || request_body
            .as_object()
            .is_some_and(|object| object.contains_key("stream"))
}


fn provider_query_key_supports_endpoint(
    key: &StoredProviderCatalogKey,
    provider_type: &str,
    endpoint_api_format: &str,
) -> bool {
    crate::handlers::shared::provider_catalog_key_supports_format(
        key,
        provider_type,
        endpoint_api_format,
    )
}

async fn provider_query_select_preferred_non_kiro_endpoint(
    state: &AdminAppState<'_>,
    provider: &StoredProviderCatalogProvider,
    endpoints: &[StoredProviderCatalogEndpoint],
    keys: &[StoredProviderCatalogKey],
    selected_key_ids: Option<&BTreeSet<String>>,
) -> Option<StoredProviderCatalogEndpoint> {
    for priority in 0..=2 {
        for endpoint in endpoints.iter().filter(|endpoint| endpoint.is_active) {
            if provider_query_model_test_endpoint_priority(
                &provider.provider_type,
                &endpoint.api_format,
            ) != Some(priority)
            {
                continue;
            }
            for key in keys {
                if !key.is_active
                    || !provider_query_selected_key_ids_allow_key(selected_key_ids, &key.id)
                    || !provider_query_key_supports_endpoint(
                        key,
                        &provider.provider_type,
                        &endpoint.api_format,
                    )
                {
                    continue;
                }
                let Ok(Some(transport)) = state
                    .read_provider_transport_snapshot(&provider.id, &endpoint.id, &key.id)
                    .await
                else {
                    continue;
                };
                if provider_query_transport_supports_model_test_execution(
                    state,
                    &transport,
                    endpoint.api_format.as_str(),
                ) {
                    return Some(endpoint.clone());
                }
            }
        }
    }

    endpoints
        .iter()
        .find(|endpoint| {
            endpoint.is_active
                && keys.iter().any(|key| {
                    key.is_active
                        && provider_query_selected_key_ids_allow_key(selected_key_ids, &key.id)
                        && provider_query_key_supports_endpoint(
                            key,
                            &provider.provider_type,
                            &endpoint.api_format,
                        )
                })
        })
        .or_else(|| endpoints.iter().find(|endpoint| endpoint.is_active))
        .cloned()
}

fn provider_query_selected_key_ids_allow_key(
    selected_key_ids: Option<&BTreeSet<String>>,
    key_id: &str,
) -> bool {
    selected_key_ids.is_none_or(|ids| ids.contains(key_id))
}

fn provider_query_selected_key_ids_all_exist(
    selected_key_ids: &BTreeSet<String>,
    keys: &[StoredProviderCatalogKey],
) -> bool {
    selected_key_ids
        .iter()
        .all(|id| keys.iter().any(|key| key.id == *id))
}

fn provider_query_model_name_matches(left: &str, right: &str) -> bool {
    let left = left.trim();
    let right = right.trim();
    !left.is_empty() && !right.is_empty() && left.eq_ignore_ascii_case(right)
}

fn provider_query_key_allows_effective_test_model(
    key: &StoredProviderCatalogKey,
    requested_model: &str,
    effective_model: &str,
) -> bool {
    let allowed_models = json_string_list(key.allowed_models.as_ref());
    if key.allowed_models.is_none() || allowed_models.is_empty() {
        return true;
    }

    let requested_base_model = crate::ai_serving::model_directive_base_model(requested_model);
    allowed_models
        .iter()
        .map(String::as_str)
        .any(|allowed_model| {
            provider_query_model_name_matches(allowed_model, requested_model)
                || provider_query_model_name_matches(allowed_model, effective_model)
                || requested_base_model.as_deref().is_some_and(|base_model| {
                    provider_query_model_name_matches(allowed_model, base_model)
                })
        })
}

fn provider_query_test_key_sort_key(
    provider_type: &str,
    key: &StoredProviderCatalogKey,
    endpoint_api_format: &str,
    now_unix_secs: u64,
) -> (u8, i32, u64) {
    let circuit_open = key
        .circuit_breaker_by_format
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get(endpoint_api_format))
        .is_some_and(|value| provider_key_circuit_payload_is_active_open_at(value, now_unix_secs));
    let health_score = key
        .health_by_format
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get(endpoint_api_format))
        .and_then(Value::as_object)
        .and_then(|value| value.get("health_score"))
        .and_then(Value::as_f64)
        .unwrap_or(1.0);
    let consecutive_failures = key
        .health_by_format
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| value.get(endpoint_api_format))
        .and_then(Value::as_object)
        .and_then(|value| value.get("consecutive_failures"))
        .and_then(Value::as_u64)
        .unwrap_or(0);
    let normalized_health = (health_score.clamp(0.0, 1.0) * 1000.0).round() as i32;

    (if circuit_open { 1 } else { 0 }, -normalized_health, consecutive_failures)
}

async fn provider_query_reconcile_fixed_provider_endpoints_for_test_model(
    _state: &AdminAppState<'_>,
    _provider: &StoredProviderCatalogProvider,
) -> Result<(), Response<Body>> {
    Ok(())
}

fn provider_query_decode_execution_body(
    result: &aether_contracts::ExecutionResult,
) -> Option<Vec<u8>> {
    result
        .body
        .as_ref()
        .and_then(|body| body.body_bytes_b64.as_deref())
        .and_then(|value| base64::engine::general_purpose::STANDARD.decode(value).ok())
}

fn provider_query_execution_json_body(result: &aether_contracts::ExecutionResult) -> Option<Value> {
    result
        .body
        .as_ref()
        .and_then(|body| body.json_body.clone())
        .or_else(|| {
            provider_query_decode_execution_body(result)
                .and_then(|body| serde_json::from_slice::<Value>(&body).ok())
        })
}

fn provider_query_aggregate_standard_stream_sync_response(
    provider_api_format: &str,
    body: &[u8],
) -> Option<Value> {
    match provider_query_normalize_api_format_alias(provider_api_format).as_str() {
        "openai:chat" => crate::ai_serving::aggregate_openai_chat_stream_sync_response(body),
        "openai:responses" | "openai:responses:compact" => {
            crate::ai_serving::aggregate_openai_responses_stream_sync_response(body)
        }
        "claude:messages" => crate::ai_serving::aggregate_claude_stream_sync_response(body),
        "gemini:generate_content" => crate::ai_serving::aggregate_gemini_stream_sync_response(body),
        _ => None,
    }
}

fn provider_query_standard_execution_response_body(
    provider_api_format: &str,
    result: &aether_contracts::ExecutionResult,
    report_context: Option<&Value>,
) -> Option<Value> {
    let body = provider_query_execution_json_body(result).or_else(|| {
        provider_query_decode_execution_body(result).and_then(|body| {
            provider_query_aggregate_standard_stream_sync_response(provider_api_format, &body)
        })
    })?;
    let body = report_context
        .and_then(|context| {
            crate::ai_serving::api::normalize_provider_private_response_value(body.clone(), context)
        })
        .unwrap_or(body);
    if result.status_code < 400
        && provider_query_normalize_api_format_alias(provider_api_format)
            == "gemini:generate_content"
        && !crate::ai_serving::gemini_generate_content_response_has_visible_output(&body)
    {
        return None;
    }
    if result.status_code < 400
        && provider_query_normalize_api_format_alias(provider_api_format) == "openai:search"
        && !body
            .get("output")
            .and_then(Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    {
        return None;
    }
    Some(body)
}

fn provider_query_extract_error_message(
    result: &aether_contracts::ExecutionResult,
) -> Option<String> {
    provider_query_execution_json_body(result)
        .as_ref()
        .and_then(Value::as_object)
        .and_then(|value| {
            value
                .get("error")
                .and_then(Value::as_object)
                .and_then(|error| error.get("message"))
                .and_then(Value::as_str)
                .or_else(|| value.get("message").and_then(Value::as_str))
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            provider_query_decode_execution_body(result)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
        })
        .or_else(|| {
            result
                .error
                .as_ref()
                .map(|error| error.message.trim().to_string())
                .filter(|value| !value.is_empty())
        })
}




fn provider_query_build_openai_image_test_request_body_for_route(
    payload: &Value,
    model: &str,
    route_path: &str,
) -> Value {
    if let Some(mut body) = provider_query_extract_request_body(payload) {
        if let Some(object) = body.as_object_mut() {
            if route_path.ends_with("/test-model-failover") {
                object.insert("model".to_string(), Value::String(model.to_string()));
            } else {
                object
                    .entry("model".to_string())
                    .or_insert_with(|| Value::String(model.to_string()));
            }
        }
        return body;
    }

    json!({
        "model": model,
        "prompt": provider_query_extract_message(payload)
            .unwrap_or_else(|| DEFAULT_PROVIDER_QUERY_TEST_MESSAGE.to_string()),
        "n": 1,
        "size": "1024x1024",
        "stream": true,
    })
}


fn provider_query_openai_image_test_upstream_url(
    transport: &AdminGatewayProviderTransportSnapshot,
    request_path: Option<&str>,
    request_query: Option<&str>,
) -> String {
    crate::provider_transport::build_openai_image_upstream_url(transport, request_path, request_query)
}

async fn provider_query_finalize_openai_image_result(
    route_path: &str,
    trace_id: &str,
    requested_model: &str,
    mapped_model: &str,
    image_request: &Value,
    result: &aether_contracts::ExecutionResult,
) -> Result<Option<Value>, GatewayError> {
    let decision = GatewayControlDecision::synthetic(
        route_path,
        Some("admin_proxy".to_string()),
        Some("provider_query_manage".to_string()),
        Some("test_model_failover".to_string()),
        Some("openai:image".to_string()),
    );
    let payload = GatewaySyncReportRequest {
        trace_id: trace_id.to_string(),
        report_kind: OPENAI_IMAGE_SYNC_FINALIZE_REPORT_KIND.to_string(),
        report_context: Some(json!({
            "client_api_format": "openai:image",
            "provider_api_format": "openai:image",
            "model": requested_model,
            "mapped_model": mapped_model,
            "needs_conversion": false,
            "has_envelope": false,
            "image_request": image_request,
        })),
        status_code: result.status_code,
        headers: result.headers.clone(),
        body_json: provider_query_execution_json_body(result),
        client_body_json: None,
        body_base64: result
            .body
            .as_ref()
            .and_then(|body| body.body_bytes_b64.clone()),
        telemetry: result.telemetry.clone(),
    };

    let Some(outcome) = maybe_build_sync_finalize_outcome(trace_id, &decision, &payload)? else {
        return Ok(None);
    };
    let bytes = to_bytes(
        outcome.response.into_body(),
        crate::headers::max_internal_buffered_body_bytes(),
    )
    .await
    .map_err(|err| GatewayError::Internal(err.to_string()))?;
    serde_json::from_slice::<Value>(&bytes)
        .map(Some)
        .map_err(|err| GatewayError::Internal(err.to_string()))
}

async fn provider_query_execute_openai_image_test_candidate(
    state: &AdminAppState<'_>,
    provider: &StoredProviderCatalogProvider,
    candidate: &ProviderQueryTestCandidate,
    payload: &Value,
    route_path: &str,
    trace_id: &str,
    requested_model: &str,
) -> Result<ProviderQueryExecutionOutcome, GatewayError> {
    let Some(mut transport) = state
        .read_provider_transport_snapshot(&provider.id, &candidate.endpoint.id, &candidate.key.id)
        .await?
    else {
        return Ok(provider_query_skipped_execution_outcome(
            Value::Null,
            "Provider transport snapshot is unavailable",
        ));
    };

    if let Some(reason) = crate::provider_transport::openai_image_transport_unsupported_reason(
        &transport,
        "openai:image",
    ) {
        let original_request_body = provider_query_build_openai_image_test_request_body_for_route(
            payload,
            &candidate.effective_model,
            route_path,
        );
        return Ok(provider_query_skipped_execution_outcome(
            original_request_body,
            format!(
                "{} ({reason})",
                provider_query_unsupported_test_api_format_message(&candidate.endpoint.api_format)
            ),
        ));
    }

    let request_body = provider_query_build_openai_image_test_request_body_for_route(
        payload,
        &candidate.effective_model,
        route_path,
    );
    let incoming_request_headers = provider_query_extract_request_headers(payload);
    let image_request_path = if request_body.get("image").is_some()
        || request_body
            .get("images")
            .and_then(Value::as_array)
            .is_some_and(|images| !images.is_empty())
    {
        "/v1/images/edits"
    } else {
        "/v1/images/generations"
    };
    let mut synthetic_request = http::Request::builder()
        .uri(image_request_path)
        .body(())
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    *synthetic_request.headers_mut() = incoming_request_headers;
    let (parts, _) = synthetic_request.into_parts();

    let provider_type = transport.provider.provider_type.as_str();
    let Some(normalized_request) = crate::ai_serving::normalize_openai_image_request_with_options(
        &parts,
        &request_body,
        None,
        provider_query_openai_image_normalize_options(
            provider_type,
            Some(candidate.effective_model.as_str()),
        ),
    ) else {
        return Ok(provider_query_skipped_execution_outcome(
            request_body.clone(),
            provider_query_openai_image_normalize_failure_message(
                provider_type,
                Some(candidate.effective_model.as_str()),
                &request_body,
            ),
        ));
    };

    let upstream_is_stream = crate::ai_serving::resolve_upstream_is_stream_for_provider(
        transport.endpoint.config.as_ref(),
        "openai:image",
        request_body
            .get("stream")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        false,
    );
    let provider_request_body = crate::ai_serving::build_openai_image_api_provider_request_body(
        &normalized_request,
        Some(candidate.effective_model.as_str()),
        upstream_is_stream,
    );
    let Some(provider_request_body) = provider_request_body else {
        return Ok(provider_query_skipped_execution_outcome(
            request_body,
            "Provider request is outside the Codex Images contract",
        ));
    };
    let Some((auth_header, auth_value)) = crate::provider_transport::resolve_openai_image_auth(&transport)
    else {
        return Ok(provider_query_skipped_execution_outcome(
            provider_request_body,
            "Provider auth is unavailable for openai:image",
        ));
    };
    let transport_profile = state.resolve_transport_profile(&transport);

    let Some(mut request_headers) = crate::provider_transport::build_openai_image_headers(
        crate::provider_transport::ProviderOpenAiImageHeadersInput {
            transport: &transport,
            headers: &parts.headers,
            auth_header: &auth_header,
            auth_value: &auth_value,
            accept: if upstream_is_stream {
                Some("text/event-stream")
            } else {
                Some("application/json")
            },
            header_rules: transport.endpoint.header_rules.as_ref(),
            provider_request_body: &provider_request_body,
            original_request_body: &request_body,
        },
    ) else {
        return Ok(ProviderQueryExecutionOutcome {
            status: "failed",
            skip_reason: None,
            error_message: Some("provider request headers build failed".to_string()),
            status_code: None,
            latency_ms: None,
            request_url: String::new(),
            request_headers: BTreeMap::new(),
            request_body: provider_request_body,
            response_headers: BTreeMap::new(),
            response_body: None,
        });
    };
    crate::provider_transport::ensure_upstream_auth_header(
        &mut request_headers,
        &auth_header,
        &auth_value,
    );

    let request_model = normalized_request
        .requested_model
        .clone()
        .unwrap_or_else(|| {
            crate::ai_serving::default_model_for_openai_image_operation(
                normalized_request.operation,
            )
            .to_string()
        });
    let mapped_model = provider_request_body
        .get("model")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(request_model.as_str())
        .to_string();
    let image_request = normalized_request.summary_json.clone();
    let request_url = provider_query_openai_image_test_upstream_url(
        &transport,
        Some(image_request_path),
        parts.uri.query(),
    );

    let plan = ExecutionPlan {
        request_id: trace_id.to_string(),
        candidate_id: Some(format!("provider-query-{}", candidate.key.id)),
        provider_name: Some(provider.name.clone()),
        provider_id: provider.id.clone(),
        endpoint_id: candidate.endpoint.id.clone(),
        key_id: candidate.key.id.clone(),
        method: "POST".to_string(),
        url: request_url.clone(),
        headers: request_headers.clone(),
        content_type: Some("application/json".to_string()),
        content_encoding: None,
        body: RequestBody::from_json(provider_request_body.clone()),
        stream: upstream_is_stream,
        client_api_format: "openai:image".to_string(),
        provider_api_format: "openai:image".to_string(),
        model_name: Some(request_model.clone()),
        proxy: state
            .resolve_transport_proxy_snapshot_with_tunnel_affinity(&transport)
            .await,
        transport_profile: transport_profile.clone(),
        timeouts: state.resolve_transport_execution_timeouts(&transport),
    };

    let result = state
        .execute_execution_runtime_sync_plan(Some(trace_id), &plan)
        .await?;
    let response_body = if result.status_code < 400 {
        provider_query_finalize_openai_image_result(
            route_path,
            trace_id,
            requested_model,
            &mapped_model,
            &image_request,
            &result,
        )
        .await?
        .or_else(|| provider_query_execution_json_body(&result))
    } else {
        provider_query_execution_json_body(&result)
    };
    let did_fail = result.status_code >= 400;
    let error_message = if did_fail {
        provider_query_extract_error_message(&result)
    } else if response_body.is_none()
        && provider_query_decode_execution_body(&result)
            .is_some_and(|body| crate::ai_serving::stream_body_contains_error_event(&body))
    {
        Some("OpenAI image upstream returned embedded stream error".to_string())
    } else {
        None
    };

    Ok(ProviderQueryExecutionOutcome {
        status: if did_fail || error_message.is_some() {
            "failed"
        } else {
            "success"
        },
        skip_reason: None,
        error_message,
        status_code: Some(result.status_code),
        latency_ms: result.telemetry.as_ref().and_then(|value| value.elapsed_ms),
        request_url,
        request_headers,
        request_body: provider_request_body,
        response_headers: result.headers,
        response_body,
    })
}




async fn provider_query_execute_standard_test_candidate(
    state: &AdminAppState<'_>,
    provider: &StoredProviderCatalogProvider,
    candidate: &ProviderQueryTestCandidate,
    payload: &Value,
    route_path: &str,
    trace_id: &str,
) -> Result<ProviderQueryExecutionOutcome, GatewayError> {
    let Some(mut transport) = state
        .read_provider_transport_snapshot(&provider.id, &candidate.endpoint.id, &candidate.key.id)
        .await?
    else {
        return Ok(provider_query_skipped_execution_outcome(
            Value::Null,
            "Provider transport snapshot is unavailable",
        ));
    };
    let provider_api_format = candidate.endpoint.api_format.as_str();
    let normalized_provider_api_format =
        crate::ai_serving::normalize_api_format_alias(provider_api_format);
    let client_api_format =
        provider_query_standard_test_client_api_format(normalized_provider_api_format.as_str());
    let original_request_body =
        provider_query_build_test_request_body_for_api_format_with_search_session(
            payload,
            &candidate.effective_model,
            route_path,
            client_api_format,
            Some(trace_id),
        );
    if crate::provider_transport::is_windsurf_provider_transport(&transport)
        && provider_query_normalize_api_format_alias(candidate.endpoint.api_format.as_str())
            == "openai:chat"
    {
        return provider_query_execute_windsurf_test_candidate(
            state,
            provider,
            candidate,
            payload,
            route_path,
            trace_id,
            transport,
            original_request_body,
        )
        .await;
    }
    if !provider_query_transport_supports_model_test_execution(
        state,
        &transport,
        provider_api_format,
    ) {
        return Ok(provider_query_skipped_execution_outcome(
            original_request_body,
            provider_query_standard_test_unsupported_reason(&transport, provider_api_format),
        ));
    }

    let incoming_request_headers = provider_query_extract_request_headers(payload);
    let mut request_body = original_request_body.clone();
    if let Some(object) = request_body.as_object_mut() {
        object.insert("stream".to_string(), Value::Bool(false));
    }
    let request_model =
        provider_query_request_body_model(&request_body, &candidate.effective_model);

    let upstream_is_stream = provider_query_resolve_standard_test_upstream_is_stream(
        transport.endpoint.config.as_ref(),
        transport.provider.provider_type.as_str(),
        provider_api_format,
    );
    let require_body_stream_field = provider_query_request_requires_body_stream_field(
        &request_body,
        transport.endpoint.config.as_ref(),
    );
    let mut provider_request_body = match normalized_provider_api_format.as_str() {
        "openai:chat" => {
            let Some(mut provider_request_body) =
                crate::ai_serving::build_local_openai_chat_request_body(
                    &request_body,
                    request_model,
                    upstream_is_stream,
                )
            else {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body could not be built for {provider_api_format}"),
                ));
            };
            if !crate::provider_transport::apply_local_body_rules_with_request_headers(
                &mut provider_request_body,
                transport.endpoint.body_rules.as_ref(),
                Some(&request_body),
                Some(&incoming_request_headers),
            ) {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body rules rejected {provider_api_format}"),
                ));
            }
            provider_request_body
        }
        "claude:messages" | "gemini:generate_content" => {
            let Some(mut provider_request_body) =
                crate::ai_serving::build_cross_format_openai_chat_request_body(
                    &request_body,
                    request_model,
                    normalized_provider_api_format.as_str(),
                    upstream_is_stream,
                )
            else {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body could not be built for {provider_api_format}"),
                ));
            };
            if !crate::provider_transport::apply_local_body_rules_with_request_headers(
                &mut provider_request_body,
                transport.endpoint.body_rules.as_ref(),
                Some(&request_body),
                Some(&incoming_request_headers),
            ) {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body rules rejected {provider_api_format}"),
                ));
            }
            provider_request_body
        }
        "openai:responses" | "openai:responses:compact" => {
            let Some(mut provider_request_body) =
                (if provider_query_request_body_is_openai_responses_shape(&request_body) {
                    crate::ai_serving::build_local_openai_responses_request_body(
                        &request_body,
                        request_model,
                        upstream_is_stream,
                    )
                } else {
                    crate::ai_serving::build_cross_format_openai_chat_request_body(
                        &request_body,
                        request_model,
                        normalized_provider_api_format.as_str(),
                        upstream_is_stream,
                    )
                })
            else {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body could not be built for {provider_api_format}"),
                ));
            };
            if !crate::provider_transport::apply_local_body_rules_with_request_headers(
                &mut provider_request_body,
                transport.endpoint.body_rules.as_ref(),
                Some(&request_body),
                Some(&incoming_request_headers),
            ) {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body rules rejected {provider_api_format}"),
                ));
            }
            crate::ai_serving::apply_codex_openai_responses_special_body_edits(
                &mut provider_request_body,
                transport.provider.provider_type.as_str(),
                provider_api_format,
                transport.endpoint.body_rules.as_ref(),
                Some(candidate.key.id.as_str()),
            );
            crate::ai_serving::apply_openai_responses_compact_special_body_edits(
                &mut provider_request_body,
                provider_api_format,
            );
            provider_request_body
        }
        "openai:search" => {
            let Some(mut provider_request_body) =
                crate::provider_transport::build_same_format_provider_request_body(
                    crate::provider_transport::SameFormatProviderRequestBodyInput {
                        body_json: &request_body,
                        mapped_model: request_model,
                        client_api_format,
                        provider_api_format,
                        source_model: request_body.get("model").and_then(Value::as_str),
                        family: crate::provider_transport::SameFormatProviderFamily::Standard,
                        body_rules: transport.endpoint.body_rules.as_ref(),
                        request_headers: Some(&incoming_request_headers),
                        upstream_is_stream,
                        force_body_stream_field: require_body_stream_field,
                        enable_model_directives: false,
                    },
                )
            else {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body could not be built for {provider_api_format}"),
                ));
            };
            if let Err(err) = crate::provider_transport::apply_transport_request_body_semantics(
                &mut provider_request_body,
                &transport,
                normalized_provider_api_format.as_str(),
            ) {
                return Ok(provider_query_skipped_execution_outcome(
                    provider_request_body,
                    format!(
                        "Provider request body is not compatible with transport semantics: {err}"
                    ),
                ));
            }
            provider_request_body
        }
        "openai:embedding"
        | "gemini:embedding"
        | "jina:embedding"
        | "doubao:embedding"
        | "aliyun:multimodal_embedding"
        | "openai:rerank"
        | "jina:rerank" => {
            let Some(mut provider_request_body) =
                crate::ai_serving::build_standard_request_body_with_model_directives_and_request_headers_and_reasoning_replay_policy(
                    &request_body,
                    client_api_format,
                    request_model,
                    transport.provider.provider_type.as_str(),
                    normalized_provider_api_format.as_str(),
                    route_path,
                    upstream_is_stream,
                    transport.endpoint.body_rules.as_ref(),
                    Some(candidate.key.id.as_str()),
                    Some(&incoming_request_headers),
                    false,
                    crate::ai_serving::openai_responses_reasoning_replay_policy(
                        transport.provider.provider_type.as_str(),
                        transport.endpoint.base_url.as_str(),
                    ),
                )
            else {
                return Ok(provider_query_skipped_execution_outcome(
                    request_body.clone(),
                    format!("Provider request body could not be built for {provider_api_format}"),
                ));
            };
            if let Err(err) = crate::provider_transport::apply_transport_request_body_semantics(
                &mut provider_request_body,
                &transport,
                normalized_provider_api_format.as_str(),
            ) {
                return Ok(provider_query_skipped_execution_outcome(
                    provider_request_body,
                    format!(
                        "Provider request body is not compatible with transport semantics: {err}"
                    ),
                ));
            }
            provider_request_body
        }
        _ => {
            return Ok(provider_query_skipped_execution_outcome(
                request_body.clone(),
                provider_query_unsupported_test_api_format_message(provider_api_format),
            ));
        }
    };
    crate::ai_serving::enforce_request_body_stream_field(
        &mut provider_request_body,
        provider_api_format,
        upstream_is_stream,
        require_body_stream_field,
    );
    let source_model = provider_query_request_body_model(&request_body, request_model);
    let codex_model_capabilities = crate::ai_serving::codex_model_capabilities_for_transport(
        &transport,
        provider_api_format,
        request_model,
        source_model,
    );
    if matches!(
        normalized_provider_api_format.as_str(),
        "openai:chat" | "openai:responses" | "openai:responses:compact" | "openai:search"
    ) && crate::ai_serving::finalize_openai_provider_request_with_codex_model_capabilities_and_reasoning_replay_policy(
        &mut provider_request_body,
        crate::ai_serving::OpenAiProviderRequestFinalization {
            source_api_format: client_api_format,
            provider_api_format,
            provider_type: transport.provider.provider_type.as_str(),
            provider_model: request_model,
            source_model,
            body_rules: transport.endpoint.body_rules.as_ref(),
            upstream_is_stream,
            require_body_stream_field,
        },
        codex_model_capabilities.as_ref(),
        crate::ai_serving::openai_responses_reasoning_replay_policy(
            transport.provider.provider_type.as_str(),
            transport.endpoint.base_url.as_str(),
        ),
    )
    .is_err()
    {
        return Ok(provider_query_skipped_execution_outcome(
            provider_request_body,
            "Provider request body violates the OpenAI provider contract",
        ));
    }
    if crate::provider_transport::is_gemini_cli_provider_transport(&transport)
        && normalized_provider_api_format == "gemini:generate_content"
    {
        let mut gemini_cli_auth =
            match crate::provider_transport::resolve_local_gemini_cli_request_auth(&transport) {
                crate::provider_transport::GeminiCliRequestAuthSupport::Supported(auth) => auth,
                crate::provider_transport::GeminiCliRequestAuthSupport::Unsupported(_) => {
                    crate::provider_transport::GeminiCliRequestAuth::default()
                }
            };
        if gemini_cli_auth.project_id.is_none() {
            gemini_cli_auth = state
                .app()
                .hydrate_gemini_cli_project_metadata_for_transport(&transport)
                .await
                .and_then(|hydrated| {
                    transport = hydrated;
                    match crate::provider_transport::resolve_local_gemini_cli_request_auth(
                        &transport,
                    ) {
                        crate::provider_transport::GeminiCliRequestAuthSupport::Supported(auth) => {
                            Some(auth)
                        }
                        crate::provider_transport::GeminiCliRequestAuthSupport::Unsupported(_) => {
                            None
                        }
                    }
                })
                .unwrap_or_default();
        }
        if gemini_cli_auth.project_id.is_none() {
            return Ok(provider_query_skipped_execution_outcome(
                provider_request_body,
                "Gemini CLI project_id is unavailable for v1internal request",
            ));
        }
        provider_request_body = match crate::provider_transport::build_gemini_cli_v1internal_request(
            &gemini_cli_auth,
            trace_id,
            request_model,
            &provider_request_body,
        ) {
            crate::provider_transport::GeminiCliRequestEnvelopeSupport::Supported(envelope) => {
                envelope
            }
            crate::provider_transport::GeminiCliRequestEnvelopeSupport::Unsupported(_) => {
                return Ok(provider_query_skipped_execution_outcome(
                    provider_request_body,
                    "Gemini CLI v1internal envelope could not be built",
                ));
            }
        };
    }
    let private_report_context =
        (crate::provider_transport::is_gemini_cli_provider_transport(&transport)
            && normalized_provider_api_format == "gemini:generate_content")
            .then(|| {
                json!({
                    "has_envelope": true,
                    "envelope_name": crate::provider_transport::GEMINI_CLI_V1INTERNAL_ENVELOPE_NAME,
                    "provider_api_format": provider_api_format,
                })
            });

    let uses_vertex_query_auth =
        crate::provider_transport::uses_vertex_api_key_query_auth(&transport, provider_api_format);
    let vertex_query_auth = if uses_vertex_query_auth {
        aether_provider_transport::vertex::resolve_local_vertex_api_key_query_auth(&transport)
    } else {
        None
    };
    let oauth_auth =
        match crate::ai_serving::normalize_api_format_alias(provider_api_format).as_str() {
            "openai:chat"
            | "openai:responses"
            | "openai:responses:compact"
            | "openai:search"
            | "claude:messages"
            | "gemini:generate_content"
            | "openai:embedding"
            | "gemini:embedding"
            | "jina:embedding"
            | "doubao:embedding"
            | "aliyun:multimodal_embedding"
            | "openai:rerank"
            | "jina:rerank" => state.resolve_local_oauth_header_auth(&transport).await?,
            _ => None,
        };
    let auth = match crate::ai_serving::normalize_api_format_alias(provider_api_format).as_str() {
        "openai:chat"
        | "openai:responses"
        | "openai:responses:compact"
        | "openai:search"
        | "openai:embedding"
        | "jina:embedding"
        | "doubao:embedding"
        | "aliyun:multimodal_embedding"
        | "openai:rerank"
        | "jina:rerank" => {
            crate::provider_transport::auth::resolve_local_openai_bearer_auth(&transport)
                .or(oauth_auth)
        }
        "claude:messages" => {
            crate::provider_transport::auth::resolve_local_standard_auth(&transport).or(oauth_auth)
        }
        "gemini:generate_content" | "gemini:embedding" => {
            if uses_vertex_query_auth {
                oauth_auth
            } else {
                state.resolve_local_gemini_auth(&transport).or(oauth_auth)
            }
        }
        _ => None,
    };
    let (auth_header, auth_value) = match auth {
        Some((auth_header, auth_value)) => (Some(auth_header), Some(auth_value)),
        None if uses_vertex_query_auth && vertex_query_auth.is_some() => (None, None),
        None => {
            return Ok(provider_query_skipped_execution_outcome(
                provider_request_body,
                format!("Provider auth is unavailable for {provider_api_format}"),
            ));
        }
    };

    let mut synthetic_request = http::Request::builder()
        .uri(route_path)
        .body(())
        .map_err(|err| GatewayError::Internal(err.to_string()))?;
    *synthetic_request.headers_mut() = incoming_request_headers;
    let (parts, _) = synthetic_request.into_parts();

    let request_url = crate::provider_transport::build_transport_request_url_for_request_body(
        &transport,
        crate::provider_transport::TransportRequestUrlParams {
            provider_api_format,
            mapped_model: Some(request_model),
            upstream_is_stream,
            request_query: parts.uri.query(),
            api_operation: None,
        },
        Some(&provider_request_body),
    );
    let Some(request_url) = request_url else {
        return Ok(provider_query_skipped_execution_outcome(
            provider_request_body,
            format!("Provider request URL is unavailable for {provider_api_format}"),
        ));
    };

    let mut request_headers = match provider_api_format {
        "claude:messages" => crate::provider_transport::auth::build_claude_passthrough_headers(
            &parts.headers,
            auth_header.as_deref().unwrap_or_default(),
            auth_value.as_deref().unwrap_or_default(),
            &BTreeMap::new(),
            Some("application/json"),
        ),
        "openai:responses" | "openai:responses:compact" | "openai:search" => {
            crate::provider_transport::auth::build_complete_passthrough_headers_with_auth(
                &parts.headers,
                auth_header.as_deref().unwrap_or_default(),
                auth_value.as_deref().unwrap_or_default(),
                &BTreeMap::new(),
                Some("application/json"),
            )
        }
        _ => match (auth_header.as_deref(), auth_value.as_deref()) {
            (Some(auth_header), Some(auth_value)) => state.build_passthrough_headers_with_auth(
                &parts.headers,
                auth_header,
                auth_value,
                &BTreeMap::new(),
            ),
            _ => crate::provider_transport::auth::build_passthrough_headers(
                &parts.headers,
                &BTreeMap::new(),
                Some("application/json"),
            ),
        },
    };
    crate::provider_transport::apply_local_auth_config_header_overrides(
        &mut request_headers,
        transport.key.decrypted_auth_config.as_deref(),
    );
    if uses_vertex_query_auth {
        request_headers.remove("x-goog-api-key");
    }
    request_headers
        .entry("content-type".to_string())
        .or_insert_with(|| "application/json".to_string());
    if crate::provider_transport::is_gemini_cli_provider_transport(&transport)
        && normalized_provider_api_format == "gemini:generate_content"
    {
        request_headers
            .entry("user-agent".to_string())
            .or_insert_with(|| crate::provider_transport::GEMINI_CLI_USER_AGENT.to_string());
    }
    let protected_headers = if uses_vertex_query_auth {
        vec!["content-type"]
    } else {
        vec![auth_header.as_deref().unwrap_or_default(), "content-type"]
    };
    if !crate::provider_transport::apply_local_header_rules_with_request_headers(
        &mut request_headers,
        transport.endpoint.header_rules.as_ref(),
        &protected_headers,
        &provider_request_body,
        Some(&request_body),
        Some(&parts.headers),
    ) {
        return Ok(ProviderQueryExecutionOutcome {
            status: "failed",
            skip_reason: None,
            error_message: Some("provider request headers build failed".to_string()),
            status_code: None,
            latency_ms: None,
            request_url,
            request_headers,
            request_body: provider_request_body,
            response_headers: BTreeMap::new(),
            response_body: None,
        });
    }
    if crate::ai_serving::is_openai_responses_family_format(provider_api_format)
        || crate::ai_serving::api_format_alias_matches(provider_api_format, "openai:search")
    {
        crate::ai_serving::apply_codex_openai_special_headers(
            &mut request_headers,
            &provider_request_body,
            &parts.headers,
            transport.provider.provider_type.as_str(),
            provider_api_format,
            Some(trace_id),
            transport.key.decrypted_auth_config.as_deref(),
        );
        let final_provider_model = provider_request_body
            .get("model")
            .and_then(Value::as_str)
            .unwrap_or(request_model);
        crate::ai_serving::apply_codex_openai_responses_lite_header_for_request_body_with_capabilities(
            &mut request_headers,
            Some(&provider_request_body),
            transport.provider.provider_type.as_str(),
            provider_api_format,
            final_provider_model,
            source_model,
            codex_model_capabilities.as_ref(),
        );
    }
    if !uses_vertex_query_auth {
        if let (Some(auth_header), Some(auth_value)) =
            (auth_header.as_deref(), auth_value.as_deref())
        {
            crate::provider_transport::ensure_upstream_auth_header(
                &mut request_headers,
                auth_header,
                auth_value,
            );
        }
    }

    let plan = ExecutionPlan {
        request_id: trace_id.to_string(),
        candidate_id: Some(format!("provider-query-{}", candidate.key.id)),
        provider_name: Some(provider.name.clone()),
        provider_id: provider.id.clone(),
        endpoint_id: candidate.endpoint.id.clone(),
        key_id: candidate.key.id.clone(),
        method: "POST".to_string(),
        url: request_url.clone(),
        headers: request_headers.clone(),
        content_type: Some("application/json".to_string()),
        content_encoding: None,
        body: RequestBody::from_json(provider_request_body.clone()),
        stream: upstream_is_stream,
        client_api_format: client_api_format.to_string(),
        provider_api_format: candidate.endpoint.api_format.clone(),
        model_name: Some(request_model.to_string()),
        proxy: state
            .resolve_transport_proxy_snapshot_with_tunnel_affinity(&transport)
            .await,
        transport_profile: state.resolve_transport_profile(&transport),
        timeouts: state.resolve_transport_execution_timeouts(&transport),
    };

    let result = state
        .execute_execution_runtime_sync_plan(Some(trace_id), &plan)
        .await?;
    let response_body = if result.status_code < 400 {
        provider_query_standard_execution_response_body(
            provider_api_format,
            &result,
            private_report_context.as_ref(),
        )
    } else {
        result.body.as_ref().and_then(|body| body.json_body.clone())
    };
    let missing_success_body = result.status_code < 400 && response_body.is_none();
    let did_fail = result.status_code >= 400 || missing_success_body;
    let error_message = if did_fail {
        provider_query_extract_error_message(&result).or_else(|| {
            missing_success_body.then(|| {
                format!(
                    "Provider returned HTTP {} without a model-test response body",
                    result.status_code
                )
            })
        })
    } else {
        None
    };

    Ok(ProviderQueryExecutionOutcome {
        status: if did_fail { "failed" } else { "success" },
        skip_reason: None,
        error_message,
        status_code: Some(result.status_code),
        latency_ms: result.telemetry.as_ref().and_then(|value| value.elapsed_ms),
        request_url,
        request_headers,
        request_body: provider_request_body,
        response_headers: result.headers,
        response_body,
    })
}

#[allow(clippy::too_many_arguments)]


pub(crate) async fn build_admin_provider_query_test_model_local_response(
    state: &AdminAppState<'_>,
    payload: &Value,
) -> Result<Response<Body>, GatewayError> {
    let response = build_admin_provider_query_kiro_failover_response(
        state,
        payload,
        "/api/admin/provider-query/test-model",
    )
    .await?;
    if !response.status().is_success() {
        return Ok(response);
    }
    let body = to_bytes(
        response.into_body(),
        crate::headers::max_internal_buffered_body_bytes(),
    )
    .await
    .map_err(|err| GatewayError::Internal(err.to_string()))?;
    let parsed: Value =
        serde_json::from_slice(&body).map_err(|err| GatewayError::Internal(err.to_string()))?;

    Ok(Json(json!({
        "success": parsed.get("success").cloned().unwrap_or(Value::Bool(false)),
        "error": parsed.get("error").cloned().unwrap_or(Value::Null),
        "data": parsed.get("data").cloned().unwrap_or(Value::Null),
        "provider": parsed.get("provider").cloned().unwrap_or(Value::Null),
        "model": parsed.get("model").cloned().unwrap_or(Value::Null),
        "attempts": parsed.get("attempts").cloned().unwrap_or_else(|| json!([])),
        "total_candidates": parsed.get("total_candidates").cloned().unwrap_or(json!(0)),
        "total_attempts": parsed.get("total_attempts").cloned().unwrap_or(json!(0)),
        "candidate_summary": parsed
            .get("candidate_summary")
            .cloned()
            .unwrap_or_else(|| provider_query_candidate_summary_payload(0, 0, &[])),
    }))
    .into_response())
}

pub(crate) async fn build_admin_provider_query_test_model_failover_local_response(
    state: &AdminAppState<'_>,
    payload: &Value,
) -> Result<Response<Body>, GatewayError> {
    build_admin_provider_query_kiro_failover_response(
        state,
        payload,
        "/api/admin/provider-query/test-model-failover",
    )
    .await
}

pub(crate) fn build_admin_provider_query_test_model_response(
    provider_id: String,
    model: String,
) -> Response<Body> {
    Json(json!({
        "success": false,
        "tested": false,
        "provider_id": provider_id,
        "model": model,
        "attempts": [],
        "total_candidates": 0,
        "total_attempts": 0,
        "candidate_summary": provider_query_candidate_summary_payload(0, 0, &[]),
        "error": ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_MESSAGE,
        "source": "local",
        "message": ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_MESSAGE,
    }))
    .into_response()
}

pub(crate) fn build_admin_provider_query_test_model_failover_response(
    provider_id: String,
    failover_models: Vec<String>,
) -> Response<Body> {
    Json(json!({
        "success": false,
        "tested": false,
        "provider_id": provider_id,
        "model": failover_models.first().cloned(),
        "failover_models": failover_models,
        "attempts": [],
        "total_candidates": 0,
        "total_attempts": 0,
        "candidate_summary": provider_query_candidate_summary_payload(0, 0, &[]),
        "error": ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_FAILOVER_MESSAGE,
        "source": "local",
        "message": ADMIN_PROVIDER_QUERY_LOCAL_TEST_MODEL_FAILOVER_MESSAGE,
    }))
    .into_response()
}

#[cfg(test)]
mod tests;
