use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use aether_usage_runtime::{build_usage_event_data_seed, UsageEvent, UsageEventType};
use axum::body::Body;
use axum::http::Response;
use serde_json::{json, Value};

use crate::ai_serving::{build_core_error_body_for_client_format, LocalCoreSyncErrorKind};
use crate::api::response::{attach_control_metadata_headers, build_client_response_from_parts};
use crate::control::GatewayControlDecision;
use crate::request_diagnostics::attach_current_request_diagnostics_and_candidate_timing_to_report_context;
use crate::{AppState, GatewayError};

const TRANSPORT_ERROR_CLIENT_MESSAGE: &str =
    "Upstream transport failed before an HTTP response was received";

#[derive(Debug, Default)]
pub(crate) struct StreamCandidateWatchdogProgress {
    terminal_started: AtomicBool,
    response_received: AtomicBool,
}

tokio::task_local! {
    static STREAM_CANDIDATE_WATCHDOG_PROGRESS: Arc<StreamCandidateWatchdogProgress>;
}

impl StreamCandidateWatchdogProgress {
    pub(crate) fn shared() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub(crate) fn terminal_started(&self) -> bool {
        self.terminal_started.load(Ordering::Acquire)
            || self.response_received.load(Ordering::Acquire)
    }

    pub(crate) async fn scope<F>(self: Arc<Self>, future: F) -> F::Output
    where
        F: Future,
    {
        STREAM_CANDIDATE_WATCHDOG_PROGRESS.scope(self, future).await
    }
}

pub(crate) fn mark_stream_candidate_response_received(status_code: u16) {
    if (200..300).contains(&status_code)
        && crate::execution_runtime::chat_retry::http_chat_stream_deadline_active()
    {
        crate::execution_runtime::chat_retry::observe_stream_response_headers();
        let _ = STREAM_CANDIDATE_WATCHDOG_PROGRESS.try_with(|progress| {
            progress.response_received.store(true, Ordering::Release);
        });
    }
}

pub(crate) fn mark_stream_candidate_watchdog_terminal_started() {
    crate::execution_runtime::chat_retry::finish_stream_request_deadline();
    crate::execution_runtime::chat_retry::finish_first_output_wait();
    let _ = STREAM_CANDIDATE_WATCHDOG_PROGRESS.try_with(|progress| {
        progress.terminal_started.store(true, Ordering::Release);
    });
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn build_transport_error_stop_response(
    state: &AppState,
    plan: &aether_contracts::ExecutionPlan,
    report_context: Option<&Value>,
    trace_id: &str,
    decision: &GatewayControlDecision,
    client_status_code: u16,
    error_type: &str,
    error_message: &str,
    elapsed_ms: u64,
) -> Result<Response<Body>, GatewayError> {
    mark_stream_candidate_watchdog_terminal_started();
    let (client_message, client_error_code) =
        if error_type == "local_stream_candidate_watchdog_timeout" {
            (
                "Stream first response timeout",
                "stream_first_response_timeout",
            )
        } else {
            (TRANSPORT_ERROR_CLIENT_MESSAGE, "upstream_transport_error")
        };
    let client_body = build_core_error_body_for_client_format(
        &plan.client_api_format,
        client_message,
        Some(client_error_code),
        LocalCoreSyncErrorKind::ServerError,
    )
    .unwrap_or_else(|| {
        json!({
            "error": {
                "type": "server_error",
                "message": client_message,
                "code": client_error_code,
            }
        })
    });
    let body_bytes =
        serde_json::to_vec(&client_body).map_err(|err| GatewayError::Internal(err.to_string()))?;
    let headers = BTreeMap::from([
        ("content-type".to_string(), "application/json".to_string()),
        ("content-length".to_string(), body_bytes.len().to_string()),
    ]);

    if state.usage_runtime.is_enabled() {
        let report_context_with_diagnostics =
            attach_current_request_diagnostics_and_candidate_timing_to_report_context(
                report_context,
                Some(elapsed_ms),
                None,
            );
        let mut usage_data = build_usage_event_data_seed(
            plan,
            report_context_with_diagnostics.as_ref().or(report_context),
        );
        usage_data.status_code = Some(client_status_code);
        usage_data.error_message = Some(error_message.to_string());
        usage_data.error_category = Some("server_error".to_string());
        usage_data.response_time_ms = Some(elapsed_ms);
        usage_data.response_headers = None;
        usage_data.response_body = None;
        usage_data.client_response_headers = Some(json!({"content-type": "application/json"}));
        usage_data.client_response_body = Some(client_body);
        let mut request_metadata = match usage_data.request_metadata.take() {
            Some(Value::Object(object)) => object,
            Some(other) => serde_json::Map::from_iter([("seed".to_string(), other)]),
            None => serde_json::Map::new(),
        };
        request_metadata.insert("transport_error".to_string(), Value::Bool(true));
        request_metadata.insert(
            "transport_error_type".to_string(),
            Value::String(error_type.to_string()),
        );
        usage_data.request_metadata = Some(Value::Object(request_metadata));
        state
            .usage_runtime
            .record_terminal_event_direct(
                state.usage_lifecycle_data_state().as_ref(),
                UsageEvent::new(UsageEventType::Failed, plan.request_id.clone(), usage_data),
            )
            .await;
    }

    attach_control_metadata_headers(
        build_client_response_from_parts(
            client_status_code,
            &headers,
            Body::from(body_bytes),
            trace_id,
            Some(decision),
        )?,
        Some(plan.request_id.as_str()),
        plan.candidate_id.as_deref(),
    )
}
