use std::collections::BTreeMap;
use std::future::Future;
use std::sync::{Arc, Mutex};

use aether_contracts::ExecutionPlan;
use aether_usage_runtime::{
    build_terminal_usage_context_seed, SyncTerminalUsagePayloadSeed, TerminalUsageContextSeed,
};
use tokio::time::{Duration, Instant};

use crate::orchestration::chat_health_policy_applies;
use crate::{AppState, GatewayError};

pub(crate) use aether_contracts::chat_retry::DEFAULT_STREAM_FAILOVER_BUDGET_MS;
const KEY_RETRY_WAIT_BUDGET: Duration = Duration::from_secs(2);

tokio::task_local! {
    static FIRST_OUTPUT_DEADLINE: Arc<FirstOutputBudget>;
    static RETRY_AFTER_OBSERVATION: Arc<Mutex<Option<Instant>>>;
    static CHAT_PROBE_SESSION: Option<Arc<ChatProbeSession>>;
}

// The response pump retains a clone until its terminal effects finish. The
// monitor owns the original guards and never reacquires a probe.
pub(crate) struct ChatProbeSession {
    stop: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    status: tokio::sync::watch::Receiver<u8>,
    terminal: std::sync::atomic::AtomicBool,
    loss_recorded: std::sync::atomic::AtomicBool,
    state: AppState,
    candidate_snapshot:
        Option<crate::request_candidate_runtime::LocalRequestCandidateStatusSnapshot>,
    started_at_unix_ms: u64,
}

impl Drop for ChatProbeSession {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.lock().expect("probe stop lock").take() {
            let _ = stop.send(());
        }
    }
}

impl ChatProbeSession {
    async fn record_loss(&self) {
        if self
            .loss_recorded
            .swap(true, std::sync::atomic::Ordering::AcqRel)
        {
            return;
        }
        if let Some(snapshot) = self.candidate_snapshot.as_ref() {
            let finished = crate::clock::current_unix_ms();
            let _ = tokio::time::timeout(Duration::from_secs(1),
                crate::request_candidate_runtime::record_local_request_candidate_status_snapshot(
                    &self.state, snapshot, aether_scheduler_core::SchedulerRequestCandidateStatusUpdate {
                        status: aether_data_contracts::repository::candidates::RequestCandidateStatus::Cancelled,
                        status_code: Some(503), error_type: Some("chat_probe_lease_lost".into()),
                        error_message: Some("Recovery probe ownership was lost".into()),
                        latency_ms: Some(finished.saturating_sub(self.started_at_unix_ms)),
                        started_at_unix_ms: Some(self.started_at_unix_ms), finished_at_unix_ms: Some(finished),
                    },
                ),
            ).await;
        }
    }
    pub(crate) async fn lost(&self) {
        let mut status = self.status.clone();
        loop {
            if *status.borrow() == 1 {
                return;
            }
            if status.changed().await.is_err() {
                if *status.borrow() == 2 {
                    std::future::pending::<()>().await;
                }
                return;
            }
        }
    }

    pub(crate) async fn finish(&self) {
        if let Some(stop) = self.stop.lock().expect("probe stop lock").take() {
            let _ = stop.send(());
        }
        let mut status = self.status.clone();
        while *status.borrow() != 2 {
            if status.changed().await.is_err() {
                break;
            }
        }
    }
}

pub(crate) fn current_chat_probe_session() -> Option<Arc<ChatProbeSession>> {
    CHAT_PROBE_SESSION.try_with(Clone::clone).ok().flatten()
}

pub(crate) fn chat_attempt_admission_held() -> bool {
    CHAT_PROBE_SESSION.try_with(|_| ()).is_ok()
}

pub(crate) fn mark_chat_attempt_terminal() {
    if let Some(session) = current_chat_probe_session() {
        session
            .terminal
            .store(true, std::sync::atomic::Ordering::Release);
    }
}

pub(crate) async fn chat_probe_session_lost(session: Option<&Arc<ChatProbeSession>>) {
    match session {
        Some(session) => session.lost().await,
        None => std::future::pending().await,
    }
}

pub(crate) async fn scope_chat_probe_session<F, T>(
    session: Option<Arc<ChatProbeSession>>,
    future: F,
) -> Result<T, GatewayError>
where
    F: Future<Output = Result<T, GatewayError>>,
{
    CHAT_PROBE_SESSION.scope(session.clone(), async {
        let mut future = Box::pin(future);
        tokio::select! {
            biased;
            result = &mut future => result,
            () = chat_probe_session_lost(session.as_ref()) => {
                if session.as_ref().is_some_and(|session| session.terminal.load(std::sync::atomic::Ordering::Acquire)) {
                    future.await
                } else {
                    drop(future);
                    if let Some(session) = session.as_ref() { session.record_loss().await; }
                    Err(GatewayError::Client {
                        status: http::StatusCode::SERVICE_UNAVAILABLE,
                        message: "Upstream recovery probe ownership was lost".to_string(),
                    })
                }
            }
        }
    }).await
}

pub(crate) fn spawn_chat_stream_pump<F>(
    failure_tx: tokio::sync::mpsc::Sender<Result<axum::body::Bytes, std::io::Error>>,
    future: F,
) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let session = current_chat_probe_session();
    tokio::spawn(async move {
        if let Err(error) = scope_chat_probe_session(session.clone(), async move {
            future.await;
            Ok(())
        })
        .await
        {
            tracing::warn!(event_name = "chat_probe_lease_lost", error = ?error,
                "stream stopped after recovery probe ownership was lost");
            let _ = tokio::time::timeout(
                Duration::from_secs(1),
                failure_tx.send(Err(std::io::Error::other(
                    "Recovery probe ownership was lost",
                ))),
            )
            .await;
        }
        if let Some(session) = session {
            session.finish().await;
        }
    })
}

pub(crate) async fn claim_chat_attempt_probes(
    state: &AppState,
    plan: &ExecutionPlan,
    report_context: &mut Option<serde_json::Value>,
) -> Result<Option<Option<Arc<ChatProbeSession>>>, GatewayError> {
    use crate::orchestration::{LocalProbeLeaseClaim, PROBE_LEASES_REPORT_FIELD};
    if !chat_health_policy_applies(&plan.client_api_format) {
        return Ok(Some(None));
    }
    let Some(key) = state
        .read_provider_catalog_keys_by_ids_strong(std::slice::from_ref(&plan.key_id))
        .await?
        .into_iter()
        .next()
    else {
        return Ok(None);
    };
    if !key.is_active
        || key.oauth_invalid_at_unix_secs.is_some()
        || key
            .expires_at_unix_secs
            .is_some_and(|expires| expires <= crate::clock::current_unix_secs())
        || (aether_scheduler_core::provider_key_health_score(&key, &plan.provider_api_format)
            .is_some_and(|health| health <= 0.0)
            && !aether_scheduler_core::is_provider_key_circuit_open(
                &key,
                &plan.provider_api_format,
            ))
    {
        return Ok(None);
    }
    let mut guards = Vec::new();
    for circuit in [true, false] {
        let claim = if circuit {
            crate::orchestration::try_claim_managed_local_circuit_probe(
                state,
                &plan.key_id,
                &plan.provider_api_format,
            )
            .await?
        } else {
            crate::orchestration::try_claim_managed_local_rate_limit_probe(
                state,
                &plan.key_id,
                &plan.provider_api_format,
            )
            .await?
        };
        match claim {
            LocalProbeLeaseClaim::NotRequired => {}
            LocalProbeLeaseClaim::Unavailable => return Ok(None),
            LocalProbeLeaseClaim::Acquired(guard) => guards.push(guard),
        }
    }
    if guards.is_empty() {
        return Ok(Some(None));
    }
    let report = report_context.get_or_insert_with(|| serde_json::json!({}));
    report[PROBE_LEASES_REPORT_FIELD] =
        serde_json::Value::Array(guards.iter().map(|guard| guard.report_context()).collect());
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let (status_tx, status) = tokio::sync::watch::channel(0);
    tokio::spawn(async move {
        {
            let losses: Vec<_> = guards
                .iter_mut()
                .map(|guard| Box::pin(guard.lost()))
                .collect();
            tokio::select! {
                _ = stopped => {},
                _ = futures_util::future::select_all(losses) => { let _ = status_tx.send(1); },
            }
        }
        for guard in guards {
            guard.finish().await;
        }
        // Preserve loss for consumers, while normal finish closes with state 2.
        if *status_tx.borrow() != 1 {
            let _ = status_tx.send(2);
        }
    });
    Ok(Some(Some(Arc::new(ChatProbeSession {
        stop: Mutex::new(Some(stop)),
        status,
        terminal: std::sync::atomic::AtomicBool::new(false),
        loss_recorded: std::sync::atomic::AtomicBool::new(false),
        state: state.clone(),
        candidate_snapshot:
            crate::request_candidate_runtime::snapshot_local_request_candidate_status(
                plan,
                report_context.as_ref(),
            ),
        started_at_unix_ms: crate::clock::current_unix_ms(),
    }))))
}

pub(crate) fn observe_chat_retry_after(plan: &ExecutionPlan, value: Option<&str>) -> Option<u64> {
    let now = crate::clock::current_unix_secs();
    if !chat_health_policy_applies(&plan.provider_api_format) {
        return crate::orchestration::parse_retry_after_secs(value, now);
    }
    let seconds = crate::orchestration::parse_chat_retry_after_secs(value, now);
    if let Some(seconds) = seconds {
        let _ = RETRY_AFTER_OBSERVATION.try_with(|observation| {
            *observation.lock().expect("retry observation lock") =
                Instant::now().checked_add(Duration::from_secs(seconds));
        });
    }
    seconds
}

struct FirstOutputBudget {
    started: Instant,
    state: Mutex<(Instant, bool, bool)>,
    changed: tokio::sync::Notify,
    active_attempt: Mutex<Option<FirstOutputAttempt>>,
}

struct FirstOutputAttempt {
    state: AppState,
    snapshot: Option<crate::request_candidate_runtime::LocalRequestCandidateStatusSnapshot>,
    started_at_unix_ms: u64,
    usage_context: TerminalUsageContextSeed,
}

// Retain only identity/routing fields for cancellation. In particular, do not clone
// or keep the conversation, upstream credentials, or encrypted compact output.
fn timeout_usage_context(
    plan: &ExecutionPlan,
    report_context: Option<&serde_json::Value>,
) -> TerminalUsageContextSeed {
    let identity_plan = ExecutionPlan {
        request_id: plan.request_id.clone(),
        candidate_id: plan.candidate_id.clone(),
        provider_name: plan.provider_name.clone(),
        provider_id: plan.provider_id.clone(),
        endpoint_id: plan.endpoint_id.clone(),
        key_id: plan.key_id.clone(),
        method: String::new(),
        url: String::new(),
        headers: BTreeMap::new(),
        content_type: None,
        content_encoding: None,
        body: aether_contracts::RequestBody {
            json_body: None,
            body_bytes_b64: None,
            body_ref: plan.body.body_ref.clone(),
        },
        stream: plan.stream,
        client_api_format: plan.client_api_format.clone(),
        provider_api_format: plan.provider_api_format.clone(),
        model_name: plan.model_name.clone(),
        proxy: None,
        transport_profile: None,
        timeouts: None,
    };
    let identity_context = serde_json::Value::Object(
        [
            "user_id",
            "api_key_id",
            "username",
            "api_key_name",
            "provider_name",
            "model",
            "mapped_model",
            "model_id",
            "global_model_id",
            "candidate_index",
            "key_name",
            "planner_kind",
            "route_family",
            "route_kind",
            "execution_path",
            "needs_conversion",
            "api_key_is_standalone",
            "request_body_ref",
            "provider_request_body_ref",
            "client_api_format",
            "provider_api_format",
        ]
        .into_iter()
        .filter_map(|key| {
            let value = report_context?.get(key)?;
            (value.is_string() || value.is_boolean() || value.is_number())
                .then(|| (key.to_string(), value.clone()))
        })
        .collect(),
    );
    let mut seed = build_terminal_usage_context_seed(&identity_plan, Some(&identity_context));
    if let Some(body) = report_context
        .and_then(|context| context.get("provider_request_body"))
        .filter(|body| !body.is_null())
        .or(plan.body.json_body.as_ref())
    {
        if let Some(operation) =
            aether_ai_formats::openai_responses_request_operation(&plan.provider_api_format, body)
        {
            seed.request_type = operation.to_string();
        }
    }
    seed
}

pub(crate) fn finish_first_output_wait() {
    let _ = FIRST_OUTPUT_DEADLINE.try_with(|budget| {
        budget.state.lock().expect("first output budget lock").2 = true;
        budget.changed.notify_one();
    });
}

pub(crate) async fn record_first_output_timeout(
    state: &AppState,
    plan: &ExecutionPlan,
    report_context: Option<&serde_json::Value>,
    error_type: &str,
    message: &str,
    fallback_elapsed_ms: u64,
) {
    let elapsed_ms = FIRST_OUTPUT_DEADLINE
        .try_with(|budget| budget.started.elapsed().as_millis() as u64)
        .unwrap_or(fallback_elapsed_ms);
    persist_first_output_timeout(
        state.clone(),
        timeout_usage_context(plan, report_context),
        error_type.to_string(),
        message.to_string(),
        elapsed_ms,
    )
    .await;
}

async fn persist_first_output_timeout(
    state: AppState,
    context_seed: TerminalUsageContextSeed,
    error_type: String,
    message: String,
    elapsed_ms: u64,
) {
    finish_first_output_wait();
    let payload = serde_json::json!({"error": {"type": error_type, "message": message}});
    let payload_seed = SyncTerminalUsagePayloadSeed {
        report_kind: "local_stream_first_output_timeout".to_string(),
        status_code: 504,
        response_time_ms: Some(elapsed_ms),
        first_byte_time_ms: None,
        provider_response_headers: None,
        client_response_headers: Some(serde_json::json!({"content-type": "application/json"})),
        provider_response_full: Some(payload.clone()),
        provider_response_body_state: None,
        client_response: Some(payload),
        client_response_body_state: None,
        standardized_usage: None,
        capture_metadata: Some(serde_json::json!({"timeout_trigger": error_type})),
    };
    let handoff = state.usage_runtime.track_persistence_handoff();
    let task = tokio::spawn(async move {
        let _handoff = handoff;
        state
            .usage_runtime
            .record_sync_terminal(
                state.usage_lifecycle_data_state().as_ref(),
                context_seed,
                payload_seed,
            )
            .await;
    });
    if let Err(error) = task.await {
        tracing::warn!(event_name = "first_output_timeout_usage_handoff_failed", %error,
            "failed to settle first-output timeout usage");
    }
}

pub(crate) fn current_first_output_deadline() -> Option<Instant> {
    FIRST_OUTPUT_DEADLINE
        .try_with(|budget| budget.state.lock().expect("first output budget lock").0)
        .ok()
}

pub(crate) fn mark_useful_stream_output() {
    crate::execution_runtime::mark_stream_candidate_watchdog_terminal_started();
    let _ = FIRST_OUTPUT_DEADLINE.try_with(|budget| {
        budget.state.lock().expect("first output budget lock").2 = true;
        budget.changed.notify_one();
    });
}

fn configure_first_output_budget(plan: &ExecutionPlan) {
    let _ = FIRST_OUTPUT_DEADLINE.try_with(|budget| {
        let mut state = budget.state.lock().expect("first output budget lock");
        if !state.1 {
            let milliseconds = plan
                .timeouts
                .as_ref()
                .and_then(|timeouts| timeouts.stream_failover_budget_ms)
                .unwrap_or(DEFAULT_STREAM_FAILOVER_BUDGET_MS);
            state.0 = budget.started + Duration::from_millis(milliseconds.max(1));
            state.1 = true;
            budget.changed.notify_one();
        }
    });
}

pub(crate) fn first_output_budget_exhausted() -> GatewayError {
    GatewayError::Client {
        status: http::StatusCode::GATEWAY_TIMEOUT,
        message: "Streaming failover budget exhausted before useful model output".to_string(),
    }
}

pub(crate) async fn capture_attempt_report_context(
    state: &AppState,
    plan: &ExecutionPlan,
    report_context: &mut Option<serde_json::Value>,
) -> Result<(), GatewayError> {
    if let Some(fence) = crate::orchestration::capture_chat_health_attempt(state, plan).await? {
        if report_context
            .as_ref()
            .and_then(|report| report.get("planned_chat_credential_fingerprint"))
            .and_then(serde_json::Value::as_str)
            .is_some_and(|planned| {
                fence
                    .get("transport_fingerprint")
                    .and_then(serde_json::Value::as_str)
                    != Some(planned)
            })
        {
            return Err(GatewayError::Client {
                status: http::StatusCode::CONFLICT,
                message: "Planned upstream credentials changed before execution".to_string(),
            });
        }
        let report = report_context.get_or_insert_with(|| serde_json::json!({}));
        if let Some(report) = report.as_object_mut() {
            report.insert(
                crate::orchestration::CHAT_HEALTH_ATTEMPT_REPORT_FIELD.to_string(),
                fence,
            );
        }
    }
    let _ = FIRST_OUTPUT_DEADLINE.try_with(|budget| {
        *budget.active_attempt.lock().expect("active attempt lock") = Some(FirstOutputAttempt {
            state: state.clone(),
            snapshot: crate::request_candidate_runtime::snapshot_local_request_candidate_status(
                plan,
                report_context.as_ref(),
            ),
            started_at_unix_ms: crate::clock::current_unix_ms(),
            usage_context: timeout_usage_context(plan, report_context.as_ref()),
        });
    });
    Ok(())
}

#[cfg(test)]
pub(crate) async fn with_first_output_deadline<F, T>(
    deadline: Instant,
    future: F,
) -> Result<T, GatewayError>
where
    F: Future<Output = Result<T, GatewayError>>,
{
    let deadline =
        current_first_output_deadline().map_or(deadline, |existing| existing.min(deadline));
    run_budget(
        Arc::new(FirstOutputBudget {
            started: Instant::now(),
            state: Mutex::new((deadline, true, false)),
            changed: tokio::sync::Notify::new(),
            active_attempt: Mutex::new(None),
        }),
        future,
    )
    .await
}

pub(crate) async fn with_stream_first_output_budget<F, T>(future: F) -> Result<T, GatewayError>
where
    F: Future<Output = Result<T, GatewayError>>,
{
    if current_first_output_deadline().is_some() {
        return future.await;
    }
    let started = Instant::now();
    run_budget(
        Arc::new(FirstOutputBudget {
            started,
            state: Mutex::new((
                started + Duration::from_millis(DEFAULT_STREAM_FAILOVER_BUDGET_MS),
                false,
                false,
            )),
            changed: tokio::sync::Notify::new(),
            active_attempt: Mutex::new(None),
        }),
        future,
    )
    .await
}

async fn settle_exhausted_first_output_budget(budget: &FirstOutputBudget) {
    let active = budget
        .active_attempt
        .lock()
        .expect("active attempt lock")
        .take();
    let deadline = budget.state.lock().expect("first output budget lock").0;
    let elapsed_ms = budget.started.elapsed().as_millis() as u64;
    tracing::warn!(
        event_name = "stream_failover_budget_exhausted",
        request_id = active
            .as_ref()
            .map(|attempt| attempt.usage_context.request_id.as_str()),
        timeout_trigger = "first_effective_output_total_budget",
        budget_ms = deadline
            .saturating_duration_since(budget.started)
            .as_millis() as u64,
        elapsed_ms,
        "stream stopped before its first effective output"
    );
    if let Some(active) = active {
        let finished = crate::clock::current_unix_ms();
        if let Some(snapshot) = active.snapshot {
            let _ = tokio::time::timeout(Duration::from_secs(1),
                crate::request_candidate_runtime::record_local_request_candidate_status_snapshot(
                    &active.state, &snapshot, aether_scheduler_core::SchedulerRequestCandidateStatusUpdate {
                        status: aether_data_contracts::repository::candidates::RequestCandidateStatus::Cancelled,
                        status_code: Some(504), error_type: Some("stream_failover_budget_exhausted".to_string()),
                        error_message: Some("Request first-output budget exhausted".to_string()),
                        latency_ms: Some(finished.saturating_sub(active.started_at_unix_ms)),
                        started_at_unix_ms: Some(active.started_at_unix_ms), finished_at_unix_ms: Some(finished),
                    },
                )).await;
        }
        persist_first_output_timeout(
            active.state,
            active.usage_context,
            "stream_failover_budget_exhausted".to_string(),
            "Streaming failover budget exhausted before useful model output".to_string(),
            elapsed_ms,
        )
        .await;
    }
}

async fn run_budget<F, T>(budget: Arc<FirstOutputBudget>, future: F) -> Result<T, GatewayError>
where
    F: Future<Output = Result<T, GatewayError>>,
{
    FIRST_OUTPUT_DEADLINE.scope(budget.clone(), async move {
        let mut future = Box::pin(future);
        loop {
            let (deadline, _, observed_output) = *budget.state.lock().expect("first output budget lock");
            if observed_output { return future.await; }
            tokio::select! {
                biased;
                result = &mut future => {
                    // prepare_attempt can observe the same deadline before the
                    // timer is polled. It has the same cancellation owner.
                    if matches!(&result, Err(GatewayError::Client { status, message })
                        if *status == http::StatusCode::GATEWAY_TIMEOUT
                        && message == "Streaming failover budget exhausted before useful model output") {
                        settle_exhausted_first_output_budget(&budget).await;
                    }
                    return result;
                },
                () = budget.changed.notified() => {},
                () = tokio::time::sleep_until(deadline) => {
                    drop(future);
                    settle_exhausted_first_output_budget(&budget).await;
                    return Err(first_output_budget_exhausted());
                },
            }
        }
    }).await
}

#[derive(Debug, Default)]
struct KeyRetryState {
    attempts: u32,
    max_attempts: Option<u32>,
    waited: Duration,
    exhausted: bool,
    retry_not_before: Option<Instant>,
}

impl KeyRetryState {
    fn reserve_wait(&mut self, required: Duration) -> bool {
        if self.exhausted || required > KEY_RETRY_WAIT_BUDGET.saturating_sub(self.waited) {
            self.exhausted = true;
            return false;
        }
        self.waited += required;
        true
    }
}

#[derive(Debug, Default)]
pub(crate) struct ChatRetryTracker {
    keys: tokio::sync::Mutex<BTreeMap<(String, String), KeyRetryState>>,
}

impl ChatRetryTracker {
    pub(crate) async fn scope_attempt<F, T>(&self, plan: &ExecutionPlan, future: F) -> T
    where
        F: Future<Output = T>,
    {
        let observation = Arc::new(Mutex::new(None));
        let output = RETRY_AFTER_OBSERVATION
            .scope(observation.clone(), future)
            .await;
        let _ = FIRST_OUTPUT_DEADLINE.try_with(|budget| {
            if let Some(active) = budget
                .active_attempt
                .lock()
                .expect("active attempt lock")
                .as_mut()
            {
                // Keep the last request identity across retry backoff/planning, but
                // never overwrite a completed candidate when the budget expires there.
                active.snapshot = None;
            }
        });
        let deadline = *observation.lock().expect("retry observation lock");
        if let Some(deadline) = deadline {
            self.keys
                .lock()
                .await
                .entry((plan.key_id.clone(), plan.provider_api_format.clone()))
                .or_default()
                .retry_not_before = Some(deadline);
        }
        output
    }
    /// Called before admission, so sleeping never owns an execution permit.
    pub(crate) async fn prepare_attempt(
        &self,
        state: &AppState,
        plan: &ExecutionPlan,
    ) -> Result<bool, GatewayError> {
        if !chat_health_policy_applies(&plan.provider_api_format) {
            return Ok(true);
        }
        configure_first_output_budget(plan);
        if plan
            .body
            .json_body
            .as_ref()
            .and_then(|body| body.get("previous_response_id"))
            .is_some_and(|reference| !reference.is_null())
        {
            return Err(GatewayError::Client {
                status: http::StatusCode::CONFLICT,
                message:
                    "HTTP Responses continuation requires a verified original upstream binding"
                        .to_string(),
            });
        }
        if current_first_output_deadline().is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(first_output_budget_exhausted());
        }
        let scope = (plan.key_id.clone(), plan.provider_api_format.clone());
        if !self.keys.lock().await.contains_key(&scope) {
            let providers = state
                .read_provider_catalog_providers_by_ids(std::slice::from_ref(&plan.provider_id))
                .await?;
            let endpoints = state
                .read_provider_catalog_endpoints_by_ids(std::slice::from_ref(&plan.endpoint_id))
                .await?;
            let provider = providers.first();
            let endpoint = endpoints.first();
            let max_attempts = (provider.is_some() || endpoint.is_some()).then(|| {
                aether_contracts::chat_retry::resolve_chat_max_attempts(
                    provider.and_then(|provider| provider.config.as_ref()),
                    endpoint.and_then(|endpoint| endpoint.config.as_ref()),
                    endpoint.and_then(|endpoint| endpoint.max_retries),
                    provider.and_then(|provider| provider.max_retries),
                )
                .max_attempts
            });
            self.keys
                .lock()
                .await
                .entry(scope.clone())
                .or_default()
                .max_attempts = max_attempts;
        }
        let (attempts, retry_not_before) = {
            let mut keys = self.keys.lock().await;
            let entry = keys.entry(scope.clone()).or_default();
            if entry.exhausted
                || entry
                    .max_attempts
                    .is_some_and(|limit| entry.attempts >= limit)
            {
                return Ok(false);
            }
            (entry.attempts, entry.retry_not_before)
        };
        if attempts > 0 {
            let cooldown = state
                .read_provider_catalog_keys_by_ids_strong(std::slice::from_ref(&plan.key_id))
                .await?
                .into_iter()
                .next()
                .and_then(|key| {
                    aether_scheduler_core::provider_key_rate_limit_cooldown(
                        &key,
                        &plan.provider_api_format,
                    )
                });
            let mut required = match cooldown {
                Some(cooldown) => Duration::from_millis(
                    cooldown
                        .until_unix_secs
                        .saturating_mul(1000)
                        .saturating_sub(crate::clock::current_unix_ms()),
                ),
                None => retry_backoff(attempts, (uuid::Uuid::new_v4().as_u128() % 501) as u64),
            };
            if let Some(deadline) = retry_not_before {
                required = required.max(deadline.saturating_duration_since(Instant::now()));
            } else if let Some(cooldown) = cooldown {
                // Missing Retry-After uses the full local ladder, not rounded wall-clock seconds.
                let seconds =
                    (1u64 << cooldown.consecutive_rate_limits.saturating_sub(1).min(6)).min(60);
                required = required.max(Duration::from_secs(seconds));
            }
            if !self
                .keys
                .lock()
                .await
                .entry(scope.clone())
                .or_default()
                .reserve_wait(required)
            {
                return Ok(false);
            }
            if !required.is_zero() {
                tokio::time::sleep(required).await;
            }
        }
        self.keys.lock().await.entry(scope).or_default().attempts = attempts.saturating_add(1);
        Ok(true)
    }
}

fn retry_backoff(previous_attempts: u32, jitter_ms: u64) -> Duration {
    let shift = previous_attempts.saturating_sub(1).min(2);
    Duration::from_millis(
        (500 + jitter_ms.min(500))
            .saturating_mul(1u64 << shift)
            .min(2_000),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wait_budget_is_cumulative_and_never_truncates_provider_wait() {
        let mut key = KeyRetryState::default();
        assert!(key.reserve_wait(Duration::from_secs(1)));
        assert!(!key.reserve_wait(Duration::from_secs(2)));
        assert_eq!(key.waited, Duration::from_secs(1));
        assert!(!key.reserve_wait(Duration::ZERO));
        assert!(!KeyRetryState::default().reserve_wait(Duration::from_secs(30)));
    }

    #[test]
    fn retry_jitter_keeps_the_approved_bounds() {
        assert_eq!(retry_backoff(1, 0), Duration::from_millis(500));
        assert_eq!(retry_backoff(1, 500), Duration::from_secs(1));
        assert_eq!(retry_backoff(2, 0), Duration::from_secs(1));
        assert_eq!(retry_backoff(99, 500), Duration::from_secs(2));
    }

    #[test]
    fn first_output_timeout_seed_preserves_compact_identity_and_body_refs_without_contents() {
        let plan: ExecutionPlan = serde_json::from_value(serde_json::json!({
            "request_id":"timeout-compact", "candidate_id":"candidate-compact",
            "provider_id":"provider", "endpoint_id":"endpoint", "key_id":"key",
            "method":"POST", "url":"https://example.invalid/v1/responses",
            "headers":{"authorization":"Bearer private-key"}, "stream":true,
            "client_api_format":"openai:responses", "provider_api_format":"openai:responses",
            "body":{"json_body":{"input":[{"type":"compaction_trigger"},
                {"role":"user", "content":"private conversation"}]},
                "body_ref":"provider-body-ref"}, "model_name":"fixture-model"
        }))
        .unwrap();
        let context = serde_json::json!({
            "user_id":"fixture-user", "api_key_id":"fixture-key", "candidate_index":2,
            "route_kind":"standard", "request_body_ref":"client-body-ref",
            "provider_request_body_ref":"provider-body-ref",
            "original_headers":{"authorization":"Bearer private-client"},
            "original_request_body":{"input":"private conversation"},
        });
        let seed = timeout_usage_context(&plan, Some(&context));
        assert_eq!(seed.request_type, "compact");
        assert_eq!(seed.user_id.as_deref(), Some("fixture-user"));
        assert!(seed.request_body.is_none());
        assert!(seed.provider_request.is_none());
        let payload = aether_usage_runtime::build_sync_terminal_usage_payload_seed(
            &aether_usage_runtime::GatewaySyncReportRequest {
                trace_id: "timeout-compact".into(),
                report_kind: "local_stream_first_output_timeout".into(),
                report_context: None,
                status_code: 504,
                headers: BTreeMap::new(),
                body_json: Some(
                    serde_json::json!({"error":{"type":"stream_failover_budget_exhausted",
                    "message":"first output timeout"}}),
                ),
                client_body_json: None,
                body_base64: None,
                telemetry: None,
            },
        );
        let event = aether_usage_runtime::build_terminal_usage_event_from_seed(
            aether_usage_runtime::build_sync_terminal_usage_seed(seed, payload),
        )
        .unwrap();
        assert_eq!(
            event.data.request_body_ref.as_deref(),
            Some("client-body-ref")
        );
        assert_eq!(
            event.data.provider_request_body_ref.as_deref(),
            Some("provider-body-ref")
        );
        assert_eq!(
            event.data.candidate_id.as_deref(),
            Some("candidate-compact")
        );
        let record = aether_usage_runtime::build_upsert_usage_record_from_event(&event).unwrap();
        assert_eq!(record.candidate_index, Some(2));
        assert_eq!(event.data.route_kind.as_deref(), Some("standard"));
        let encoded = serde_json::to_string(&event).unwrap();
        assert!(!encoded.contains("private-"));
        assert!(!encoded.contains("private conversation"));
    }

    #[tokio::test]
    async fn first_output_budget_does_not_cancel_terminal_persistence_handoff() {
        let outcome =
            with_first_output_deadline(Instant::now() + Duration::from_millis(10), async {
                crate::execution_runtime::mark_stream_candidate_watchdog_terminal_started();
                tokio::time::sleep(Duration::from_millis(30)).await;
                Ok(())
            })
            .await;
        assert!(outcome.is_ok());
    }

    #[tokio::test]
    async fn nested_candidate_budget_cannot_extend_request_deadline() {
        let started = Instant::now();
        let outcome = with_first_output_deadline(started + Duration::from_millis(20), async {
            with_first_output_deadline(started + Duration::from_secs(1), async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok(())
            })
            .await
        })
        .await;
        assert!(matches!(
            outcome,
            Err(GatewayError::Client {
                status: http::StatusCode::GATEWAY_TIMEOUT,
                ..
            })
        ));
        assert!(started.elapsed() < Duration::from_millis(500));
    }
}
