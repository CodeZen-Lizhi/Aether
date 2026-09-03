use aether_contracts::ExecutionPlan;
use tracing::{info, warn};

use crate::log_ids::short_request_id;

#[derive(Clone)]
pub(crate) struct UpstreamAttemptLog {
    trace_id: String,
    request_id: String,
    candidate_id: String,
    provider_name: String,
    endpoint_id: String,
    key_id: String,
    model_name: String,
    candidate_index: String,
    plan_kind: String,
    execution_mode: &'static str,
}

impl UpstreamAttemptLog {
    pub(crate) fn new(
        trace_id: &str,
        plan: &ExecutionPlan,
        plan_kind: &str,
        candidate_index: &str,
        execution_mode: &'static str,
    ) -> Self {
        Self {
            trace_id: trace_id.to_string(),
            request_id: short_request_id(plan.request_id.as_str()),
            candidate_id: plan.candidate_id.clone().unwrap_or_else(|| "-".to_string()),
            provider_name: plan
                .provider_name
                .clone()
                .unwrap_or_else(|| "-".to_string()),
            endpoint_id: plan.endpoint_id.clone(),
            key_id: plan.key_id.clone(),
            model_name: plan.model_name.clone().unwrap_or_else(|| "-".to_string()),
            candidate_index: candidate_index.to_string(),
            plan_kind: plan_kind.to_string(),
            execution_mode,
        }
    }
}

pub(crate) fn log_upstream_attempt_started(context: &UpstreamAttemptLog) {
    info!(
        event_name = "upstream_attempt_started",
        log_type = "event",
        trace_id = %context.trace_id,
        request_id = %context.request_id,
        candidate_id = context.candidate_id.as_str(),
        provider_name = context.provider_name.as_str(),
        endpoint_id = context.endpoint_id.as_str(),
        key_id = context.key_id.as_str(),
        model_name = context.model_name.as_str(),
        candidate_index = context.candidate_index.as_str(),
        plan_kind = context.plan_kind.as_str(),
        execution_mode = context.execution_mode,
        "gateway started upstream request"
    );
}

pub(crate) fn log_upstream_response_headers_received(
    context: &UpstreamAttemptLog,
    status_code: u16,
    upstream_ttfb_ms: u64,
) {
    info!(
        event_name = "upstream_response_headers_received",
        log_type = "event",
        trace_id = %context.trace_id,
        request_id = %context.request_id,
        candidate_id = context.candidate_id.as_str(),
        provider_name = context.provider_name.as_str(),
        endpoint_id = context.endpoint_id.as_str(),
        key_id = context.key_id.as_str(),
        model_name = context.model_name.as_str(),
        candidate_index = context.candidate_index.as_str(),
        plan_kind = context.plan_kind.as_str(),
        status_code,
        upstream_ttfb_ms,
        execution_mode = context.execution_mode,
        "gateway received upstream response headers"
    );
}

pub(crate) fn log_upstream_response_completed(
    context: &UpstreamAttemptLog,
    status_code: u16,
    upstream_elapsed_ms: u64,
) {
    info!(
        event_name = "upstream_response_completed",
        log_type = "event",
        trace_id = %context.trace_id,
        request_id = %context.request_id,
        candidate_id = context.candidate_id.as_str(),
        provider_name = context.provider_name.as_str(),
        endpoint_id = context.endpoint_id.as_str(),
        key_id = context.key_id.as_str(),
        model_name = context.model_name.as_str(),
        candidate_index = context.candidate_index.as_str(),
        plan_kind = context.plan_kind.as_str(),
        status_code,
        upstream_elapsed_ms,
        execution_mode = context.execution_mode,
        "gateway completed upstream request"
    );
}

pub(crate) fn log_upstream_request_failed(context: &UpstreamAttemptLog, upstream_elapsed_ms: u64) {
    warn!(
        event_name = "upstream_request_failed",
        log_type = "ops",
        trace_id = %context.trace_id,
        request_id = %context.request_id,
        candidate_id = context.candidate_id.as_str(),
        provider_name = context.provider_name.as_str(),
        endpoint_id = context.endpoint_id.as_str(),
        key_id = context.key_id.as_str(),
        model_name = context.model_name.as_str(),
        candidate_index = context.candidate_index.as_str(),
        plan_kind = context.plan_kind.as_str(),
        upstream_elapsed_ms,
        execution_mode = context.execution_mode,
        "gateway upstream request failed"
    );
}
