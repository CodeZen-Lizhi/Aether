use aether_contracts::{ExecutionPlan, ExecutionResult};
use aether_provider_transport::GatewayProviderTransportSnapshot;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

use crate::logic::{aggregate_models_for_cache, extract_error_message, parse_models_response_page};
use crate::transport::{build_standard_models_fetch_execution_plan, ModelFetchTransportRuntime};

#[derive(Debug, Clone, PartialEq)]
pub struct ModelsFetchOutcome {
    pub fetched_model_ids: Vec<String>,
    /// Provider response cards.
    pub cached_models: Vec<Value>,
    /// Provider cards projected into Aether's legacy admin/runtime-cache shape.
    pub legacy_models: Vec<Value>,
    pub errors: Vec<String>,
    pub has_success: bool,
    pub upstream_metadata: Option<Value>,
    pub etag: Option<String>,
    pub upstream_status: Option<u16>,
}

#[derive(Debug)]
struct ConsistentValue<T> {
    value: Option<T>,
    observed: bool,
    consistent: bool,
}

impl<T> Default for ConsistentValue<T> {
    fn default() -> Self {
        Self {
            value: None,
            observed: false,
            consistent: true,
        }
    }
}

impl<T: PartialEq> ConsistentValue<T> {
    fn observe(&mut self, candidate: Option<T>) {
        if !self.observed {
            self.consistent = candidate.is_some();
            self.value = candidate;
            self.observed = true;
            return;
        }
        if self.value.as_ref() != candidate.as_ref() {
            self.consistent = false;
            self.value = None;
        }
    }

    fn finish(self) -> Option<T> {
        (self.observed && self.consistent)
            .then_some(self.value)
            .flatten()
    }
}

pub async fn fetch_models_from_transports(
    runtime: &(impl ModelFetchTransportRuntime + ?Sized),
    transports: &[GatewayProviderTransportSnapshot],
) -> Result<ModelsFetchOutcome, String> {
    if transports.is_empty() {
        return Err("No transport snapshots available for models fetch".to_string());
    }
    fetch_standard_models(runtime, transports).await
}

async fn fetch_standard_models(
    runtime: &(impl ModelFetchTransportRuntime + ?Sized),
    transports: &[GatewayProviderTransportSnapshot],
) -> Result<ModelsFetchOutcome, String> {
    let mut all_models = Vec::new();
    let mut errors = Vec::new();
    let mut has_success = false;
    let mut etag = ConsistentValue::default();
    let mut upstream_status = ConsistentValue::default();

    for transport in transports {
        match fetch_standard_models_for_transport(runtime, transport).await {
            Ok(outcome) => {
                all_models.extend(outcome.cached_models.iter().cloned());
                has_success |= outcome.has_success;
                if outcome.has_success {
                    etag.observe(outcome.etag);
                    upstream_status.observe(outcome.upstream_status);
                }
            }
            Err((err, status)) => {
                upstream_status.observe(status);
                errors.push(format!("{}: {err}", transport.endpoint.api_format.trim()));
            }
        }
    }

    let merged_models = aggregate_models_for_cache(&all_models);
    let outcome = build_success_outcome(merged_models, None, has_success);
    Ok(outcome
        .with_errors(errors)
        .with_etag(etag.finish())
        .with_upstream_status(upstream_status.finish()))
}

async fn fetch_standard_models_for_transport(
    runtime: &(impl ModelFetchTransportRuntime + ?Sized),
    transport: &GatewayProviderTransportSnapshot,
) -> Result<ModelsFetchOutcome, (String, Option<u16>)> {
    let mut all_models = Vec::new();
    let mut seen_ids = BTreeSet::new();
    let mut next_after_id = None;
    let mut has_success = false;
    let mut etag = ConsistentValue::default();
    let mut upstream_status = ConsistentValue::default();

    for _ in 0..20 {
        let plan = build_standard_models_fetch_execution_plan(
            runtime,
            transport,
            next_after_id.as_deref(),
        )
        .await
        .map_err(|err| (err, None))?;
        let result = runtime
            .execute_model_fetch_execution_plan(&plan)
            .await
            .map_err(|err| (err, None))?;
        upstream_status.observe(Some(result.status_code));
        let body_json =
            execution_result_json_body(&result).map_err(|err| (err, Some(result.status_code)))?;
        let parsed = parse_models_response_page(&transport.endpoint.api_format, &body_json)
            .map_err(|err| (err, Some(result.status_code)))?;
        etag.observe(execution_result_header(&result, "etag"));
        has_success = true;
        for model in parsed.cached_models {
            let Some(model_id) = model
                .get("id")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
            else {
                continue;
            };
            if !seen_ids.insert(model_id.to_string()) {
                continue;
            }
            all_models.push(model);
        }

        let Some(next_cursor) = parsed
            .has_more
            .then_some(parsed.next_after_id)
            .flatten()
            .filter(|value| next_after_id.as_deref() != Some(value.as_str()))
        else {
            break;
        };
        next_after_id = Some(next_cursor);
    }

    Ok(build_success_outcome(all_models, None, has_success)
        .with_etag(etag.finish())
        .with_upstream_status(upstream_status.finish()))
}

fn execution_result_json_body(result: &ExecutionResult) -> Result<Value, String> {
    if result.status_code != 200 {
        return Err(execution_result_error_message(result));
    }
    execution_result_json_body_allow_empty(result)
}

fn execution_result_json_body_allow_empty(result: &ExecutionResult) -> Result<Value, String> {
    result
        .body
        .as_ref()
        .and_then(|body| body.json_body.clone())
        .ok_or_else(|| "models fetch response body is missing JSON payload".to_string())
}

fn execution_result_header(result: &ExecutionResult, name: &str) -> Option<String> {
    result
        .headers
        .iter()
        .find(|(header_name, _)| header_name.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.trim())
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn execution_result_error_message(result: &ExecutionResult) -> String {
    let detail = result
        .body
        .as_ref()
        .and_then(|body| body.json_body.as_ref())
        .and_then(extract_error_message)
        .or_else(|| {
            result.error.as_ref().and_then(|error| {
                let message = error.message.trim();
                (!message.is_empty()).then_some(message.to_string())
            })
        });
    match detail {
        Some(detail) if !(200..300).contains(&result.status_code) => {
            format!("HTTP {}: {detail}", result.status_code)
        }
        Some(detail) => detail,
        None => format!("HTTP {}: upstream request failed", result.status_code),
    }
}

fn build_success_outcome(
    cached_models: Vec<Value>,
    upstream_metadata: Option<Value>,
    has_success: bool,
) -> ModelsFetchOutcome {
    let legacy_models = cached_models.clone();
    ModelsFetchOutcome {
        fetched_model_ids: collect_model_ids(&cached_models),
        cached_models,
        legacy_models,
        errors: Vec::new(),
        has_success,
        upstream_metadata,
        etag: None,
        upstream_status: None,
    }
}

fn collect_model_ids(models: &[Value]) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut ids = Vec::new();
    for model in models {
        let Some(model_id) = model
            .get("id")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        if seen.insert(model_id.to_string()) {
            ids.push(model_id.to_string());
        }
    }
    ids
}

trait OutcomeExt {
    fn with_errors(self, errors: Vec<String>) -> Self;
    fn with_etag(self, etag: Option<String>) -> Self;
    fn with_upstream_status(self, upstream_status: Option<u16>) -> Self;
}

impl OutcomeExt for ModelsFetchOutcome {
    fn with_errors(mut self, errors: Vec<String>) -> Self {
        self.errors = errors;
        self
    }

    fn with_etag(mut self, etag: Option<String>) -> Self {
        self.etag = etag;
        self
    }

    fn with_upstream_status(mut self, upstream_status: Option<u16>) -> Self {
        self.upstream_status = upstream_status;
        self
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    use aether_contracts::{ExecutionResult, ResponseBody};
    use aether_provider_transport::snapshot::{
        GatewayProviderTransportEndpoint, GatewayProviderTransportKey,
        GatewayProviderTransportProvider, GatewayProviderTransportSnapshot,
    };
    use async_trait::async_trait;
    use serde_json::{json, Value};

    use super::fetch_models_from_transports;
    use crate::transport::ModelFetchTransportRuntime;

    type RouteResult = Result<(u16, Value), String>;
    type ModelFetchRoute = (String, RouteResult);

    struct RoutingTestRuntime {
        executed_urls: Arc<Mutex<Vec<String>>>,
        routes: Vec<ModelFetchRoute>,
    }

    #[async_trait]
    impl ModelFetchTransportRuntime for RoutingTestRuntime {
        async fn resolve_model_fetch_proxy(
            &self,
            _transport: &GatewayProviderTransportSnapshot,
        ) -> Option<aether_contracts::ProxySnapshot> {
            None
        }

        async fn execute_model_fetch_execution_plan(
            &self,
            plan: &aether_contracts::ExecutionPlan,
        ) -> Result<ExecutionResult, String> {
            self.executed_urls
                .lock()
                .expect("executed_urls lock")
                .push(plan.url.clone());
            let Some((_, route_result)) = self
                .routes
                .iter()
                .find(|(url_part, _)| plan.url.contains(url_part))
            else {
                return Err(format!("unexpected models fetch URL {}", plan.url));
            };
            let (status_code, response_body) = match route_result {
                Ok((status_code, response_body)) => (*status_code, response_body.clone()),
                Err(err) => return Err(err.clone()),
            };
            Ok(ExecutionResult {
                request_id: plan.request_id.clone(),
                candidate_id: plan.candidate_id.clone(),
                status_code,
                headers: BTreeMap::new(),
                response_observation: None,
                body: Some(ResponseBody {
                    json_body: Some(response_body),
                    body_bytes_b64: None,
                }),
                telemetry: None,
                error: None,
            })
        }
    }

    fn sample_transport(api_format: &str, base_url: &str) -> GatewayProviderTransportSnapshot {
        GatewayProviderTransportSnapshot {
            provider: GatewayProviderTransportProvider {
                id: "provider-1".to_string(),
                name: "Provider One".to_string(),
                provider_type: "custom".to_string(),
                website: None,
                is_active: true,
                enable_format_conversion: false,
                concurrent_limit: None,
                max_retries: None,
                proxy: None,
                request_timeout_secs: None,
                stream_first_byte_timeout_secs: None,
                config: None,
            },
            endpoint: GatewayProviderTransportEndpoint {
                id: "endpoint-1".to_string(),
                provider_id: "provider-1".to_string(),
                api_format: api_format.to_string(),
                api_family: None,
                endpoint_kind: None,
                is_active: true,
                base_url: base_url.to_string(),
                header_rules: None,
                body_rules: None,
                max_retries: None,
                custom_path: None,
                config: None,
                format_acceptance_config: None,
                proxy: None,
            },
            key: GatewayProviderTransportKey {
                id: "key-1".to_string(),
                provider_id: "provider-1".to_string(),
                name: "key".to_string(),
                auth_type: "api_key".to_string(),
                is_active: true,
                api_formats: Some(vec![api_format.to_string()]),
                auth_type_by_format: None,
                allow_auth_channel_mismatch_formats: None,
                allowed_models: None,
                capabilities: None,
                rate_multipliers: None,
                expires_at_unix_secs: None,
                proxy: None,
                fingerprint: None,
                upstream_metadata: None,
                decrypted_api_key: "secret".to_string(),
                decrypted_auth_config: None,
            },
        }
    }

    #[tokio::test]
    async fn fetches_and_aggregates_models_across_openai_transports() {
        let runtime = RoutingTestRuntime {
            executed_urls: Arc::new(Mutex::new(Vec::new())),
            routes: vec![(
                "https://one.example.com".to_string(),
                Ok((200, json!({"data": [{"id": "gpt-5"}, {"id": "gpt-4.1"}]}))),
            )],
        };
        let transports = vec![sample_transport(
            "openai:chat",
            "https://one.example.com/v1",
        )];

        let outcome = fetch_models_from_transports(&runtime, &transports)
            .await
            .expect("models fetch should succeed");

        assert!(outcome.has_success);
        assert_eq!(outcome.fetched_model_ids, vec!["gpt-4.1", "gpt-5"]);
        assert!(outcome.errors.is_empty());
    }

    #[tokio::test]
    async fn reports_upstream_errors_without_failing_the_batch() {
        let runtime = RoutingTestRuntime {
            executed_urls: Arc::new(Mutex::new(Vec::new())),
            routes: vec![(
                "https://broken.example.com".to_string(),
                Ok((500, json!({"error": {"message": "boom"}}))),
            )],
        };
        let transports = vec![sample_transport(
            "openai:chat",
            "https://broken.example.com/v1",
        )];

        let outcome = fetch_models_from_transports(&runtime, &transports)
            .await
            .expect("batch outcome should still be produced");

        assert!(!outcome.has_success);
        assert!(outcome.fetched_model_ids.is_empty());
        assert!(outcome.errors[0].contains("boom"));
    }
}
