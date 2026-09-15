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
    static STREAM_REQUEST_DEADLINE: Arc<StreamRequestDeadline>;
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
    suspend_stream_deadline_for_attempt_terminal();
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

type StreamBodySender = tokio::sync::mpsc::Sender<Result<axum::body::Bytes, std::io::Error>>;

pub(crate) fn spawn_chat_stream_pump<F>(
    failure_tx: StreamBodySender,
    timeout_event: Option<axum::body::Bytes>,
    future: F,
) -> tokio::task::JoinHandle<()>
where
    F: Future<Output = ()> + Send + 'static,
{
    let session = current_chat_probe_session();
    let budget = STREAM_REQUEST_DEADLINE.try_with(Clone::clone).ok();
    let mut failure_tx = Some(failure_tx);
    if let Some(budget) = budget.as_ref() {
        let mut state = budget.state.lock().expect("stream deadline lock");
        state.pump_owned = true;
        if !state.finished {
            *budget.failure_sender.lock().expect("stream sender lock") = failure_tx.take();
        } else {
            failure_tx.take();
        }
        budget.changed.notify_one();
    }
    tokio::spawn(async move {
        let generation = scope_chat_probe_session(session.clone(), async move {
            future.await;
            Ok(())
        });
        let result = if let Some(budget) = budget.as_ref() {
            run_stream_request_deadline(budget.clone(), true, generation).await
        } else {
            generation.await
        };
        if let Err(error) = result {
            tracing::warn!(event_name = "chat_stream_cancelled", error = ?error,
                "stream stopped after request deadline or recovery probe ownership loss");
            let total_timeout = matches!(&error, GatewayError::Client { message, .. }
                if message == STREAM_TOTAL_TIMEOUT_MESSAGE);
            let failure = if total_timeout {
                timeout_event
                    .map(Ok)
                    .unwrap_or_else(|| Err(std::io::Error::other(STREAM_TOTAL_TIMEOUT_MESSAGE)))
            } else {
                Err(std::io::Error::other("Recovery probe ownership was lost"))
            };
            let failure_tx = budget
                .as_ref()
                .and_then(|budget| {
                    budget
                        .failure_sender
                        .lock()
                        .expect("stream sender lock")
                        .take()
                })
                .or(failure_tx);
            if let Some(failure_tx) = failure_tx {
                let _ =
                    tokio::time::timeout(Duration::from_secs(1), failure_tx.send(failure)).await;
            }
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

// HTTP chat streams own a deadline through response-body delivery. The request
// future transfers ownership to the pump before returning a Response; the two
// futures must never independently settle the same timeout.
struct StreamRequestDeadline {
    started: Instant,
    state: Mutex<StreamRequestDeadlineState>,
    changed: tokio::sync::Notify,
    active_attempt: Mutex<Option<FirstOutputAttempt>>,
    release_resources: Mutex<Vec<Box<dyn FnOnce() + Send>>>,
    failure_sender: Mutex<Option<StreamBodySender>>,
}

struct StreamRequestDeadlineState {
    deadline: Instant,
    configured: bool,
    pump_owned: bool,
    terminal_persistence: bool,
    finished: bool,
    timing: StreamResponseTiming,
}

#[derive(Default, serde::Serialize)]
struct StreamResponseTiming {
    response_headers_elapsed_ms: Option<u64>,
    first_body_elapsed_ms: Option<u64>,
    first_effective_output_elapsed_ms: Option<u64>,
    first_byte_ms: Option<u64>,
}

pub(crate) fn observe_stream_response_headers() {
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        budget
            .state
            .lock()
            .expect("stream deadline lock")
            .timing
            .response_headers_elapsed_ms
            .get_or_insert(budget.started.elapsed().as_millis() as u64);
    });
}

pub(crate) fn observe_stream_first_body(first_byte_ms: Option<u64>) {
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        let mut state = budget.state.lock().expect("stream deadline lock");
        state
            .timing
            .first_body_elapsed_ms
            .get_or_insert(budget.started.elapsed().as_millis() as u64);
        state.timing.first_byte_ms = state.timing.first_byte_ms.or(first_byte_ms);
    });
}

// Body drop and request timeout race to take the same permit. A weak release
// hook lets the deadline free capacity even if the client stops reading its body.
pub(crate) struct StreamDeadlinePermit<T>(Arc<Mutex<Option<T>>>);

impl<T> Drop for StreamDeadlinePermit<T> {
    fn drop(&mut self) {
        self.0.lock().expect("stream permit lock").take();
    }
}

pub(crate) fn hold_stream_deadline_permit<T: Send + 'static>(permit: T) -> StreamDeadlinePermit<T> {
    let guard = StreamDeadlinePermit(Arc::new(Mutex::new(Some(permit))));
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        let state = budget.state.lock().expect("stream deadline lock");
        if state.finished {
            guard.0.lock().expect("stream permit lock").take();
        } else {
            let permit = Arc::downgrade(&guard.0);
            budget
                .release_resources
                .lock()
                .expect("stream resources lock")
                .push(Box::new(move || {
                    if let Some(permit) = permit.upgrade() {
                        permit.lock().expect("stream permit lock").take();
                    }
                }));
        }
    });
    guard
}

fn release_stream_deadline_resources(budget: &StreamRequestDeadline) {
    let releases = std::mem::take(
        &mut *budget
            .release_resources
            .lock()
            .expect("stream resources lock"),
    );
    for release in releases {
        release();
    }
}

pub(crate) const STREAM_TOTAL_TIMEOUT_MESSAGE: &str = "Streaming request total timeout exceeded";

pub(crate) fn http_chat_stream_deadline_active() -> bool {
    STREAM_REQUEST_DEADLINE.try_with(|_| ()).is_ok()
}

pub(crate) fn wait_for_useful_chat_output() -> bool {
    http_chat_stream_deadline_active() || current_first_output_deadline().is_some()
}

fn stream_total_timeout_error() -> GatewayError {
    GatewayError::Client {
        status: http::StatusCode::GATEWAY_TIMEOUT,
        message: STREAM_TOTAL_TIMEOUT_MESSAGE.to_string(),
    }
}

// This is called only after generation and downstream sends finish. Persistence
// itself must remain runnable after the generation deadline expires.
pub(crate) fn finish_stream_request_deadline() {
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        budget.state.lock().expect("stream deadline lock").finished = true;
        release_stream_deadline_resources(budget);
        // Closing the response must not await slow terminal KV/usage writes.
        budget
            .failure_sender
            .lock()
            .expect("stream sender lock")
            .take();
        budget.changed.notify_one();
    });
}

pub(super) fn suspend_stream_deadline_for_attempt_terminal() {
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        let mut state = budget.state.lock().expect("stream deadline lock");
        if !state.pump_owned {
            state.terminal_persistence = true;
            budget.changed.notify_one();
        }
    });
}

fn configure_stream_request_deadline(state: &AppState, plan: &ExecutionPlan) {
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        let mut deadline = budget.state.lock().expect("stream deadline lock");
        deadline.terminal_persistence = false;
        if !deadline.configured {
            let milliseconds = plan
                .timeouts
                .as_ref()
                .and_then(|timeouts| timeouts.stream_total_ms)
                .unwrap_or(aether_contracts::chat_retry::DEFAULT_STREAM_TOTAL_TIMEOUT_MS);
            deadline.deadline = budget.started + Duration::from_millis(milliseconds.max(1));
            deadline.configured = true;
            *budget.active_attempt.lock().expect("active attempt lock") =
                Some(FirstOutputAttempt {
                    state: state.clone(),
                    snapshot: None,
                    started_at_unix_ms: crate::clock::current_unix_ms(),
                    usage_context: timeout_usage_context(plan, None),
                });
        }
        budget.changed.notify_one();
    });
}

pub(crate) async fn with_stream_request_timeout<F, T>(future: F) -> Result<T, GatewayError>
where
    F: Future<Output = Result<T, GatewayError>>,
{
    if http_chat_stream_deadline_active() {
        return future.await;
    }
    let started = crate::request_diagnostics::current_request_diagnostics()
        .and_then(|diagnostics| diagnostics.request_accepted_at())
        .map(Instant::from_std)
        .unwrap_or_else(Instant::now);
    let budget = Arc::new(StreamRequestDeadline {
        started,
        state: Mutex::new(StreamRequestDeadlineState {
            deadline: started
                + Duration::from_millis(
                    aether_contracts::chat_retry::DEFAULT_STREAM_TOTAL_TIMEOUT_MS,
                ),
            configured: false,
            pump_owned: false,
            terminal_persistence: false,
            finished: false,
            timing: StreamResponseTiming::default(),
        }),
        changed: tokio::sync::Notify::new(),
        active_attempt: Mutex::new(None),
        release_resources: Mutex::new(Vec::new()),
        failure_sender: Mutex::new(None),
    });
    run_stream_request_deadline(budget, false, future).await
}

async fn run_stream_request_deadline<F, T>(
    budget: Arc<StreamRequestDeadline>,
    pump: bool,
    future: F,
) -> Result<T, GatewayError>
where
    F: Future<Output = Result<T, GatewayError>>,
{
    STREAM_REQUEST_DEADLINE
        .scope(budget.clone(), async move {
            let mut future = Box::pin(future);
            loop {
                let (deadline, monitor, finished, transferred) = {
                    let state = budget.state.lock().expect("stream deadline lock");
                    (
                        state.deadline,
                        state.pump_owned == pump && !state.terminal_persistence,
                        state.finished,
                        !pump && state.pump_owned,
                    )
                };
                if finished || transferred {
                    return future.await;
                }
                tokio::select! {
                    biased;
                    result = &mut future => return result,
                    () = budget.changed.notified() => {},
                    () = tokio::time::sleep_until(deadline), if monitor => {
                        // Generation may have transferred ownership or started
                        // persistence during this select poll. Only the current
                        // owner may claim cancellation under the shared lock.
                        {
                            let mut state = budget.state.lock().expect("stream deadline lock");
                            if state.finished || state.pump_owned != pump || state.terminal_persistence
                                || Instant::now() < state.deadline {
                                continue;
                            }
                            state.finished = true;
                        }
                        release_stream_deadline_resources(&budget);
                        drop(future);
                        settle_stream_request_timeout(&budget).await;
                        return Err(stream_total_timeout_error());
                    },
                }
            }
        })
        .await
}

async fn settle_stream_request_timeout(budget: &StreamRequestDeadline) {
    let active = budget
        .active_attempt
        .lock()
        .expect("active attempt lock")
        .take();
    let elapsed_ms = budget.started.elapsed().as_millis() as u64;
    tracing::warn!(
        event_name = "stream_total_timeout",
        elapsed_ms,
        request_id = active
            .as_ref()
            .map(|attempt| attempt.usage_context.request_id.as_str()),
        "stream exceeded its logical request deadline"
    );
    if let Some(active) = active {
        let finished = crate::clock::current_unix_ms();
        if let Some(snapshot) = active.snapshot {
            crate::request_candidate_runtime::record_local_request_candidate_status_snapshot(
                &active.state, &snapshot, aether_scheduler_core::SchedulerRequestCandidateStatusUpdate {
                    status: aether_data_contracts::repository::candidates::RequestCandidateStatus::Failed,
                    status_code: Some(504), error_type: Some("stream_total_timeout".to_string()),
                    error_message: Some(STREAM_TOTAL_TIMEOUT_MESSAGE.to_string()),
                    latency_ms: Some(finished.saturating_sub(active.started_at_unix_ms)),
                    started_at_unix_ms: Some(active.started_at_unix_ms), finished_at_unix_ms: Some(finished),
                },
            ).await;
        }
        persist_first_output_timeout(
            active.state,
            active.usage_context,
            "stream_total_timeout".to_string(),
            STREAM_TOTAL_TIMEOUT_MESSAGE.to_string(),
            elapsed_ms,
        )
        .await;
    }
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
    finish_stream_request_deadline();
    let elapsed_ms = STREAM_REQUEST_DEADLINE
        .try_with(|budget| budget.started.elapsed().as_millis() as u64)
        .or_else(|_| {
            FIRST_OUTPUT_DEADLINE.try_with(|budget| budget.started.elapsed().as_millis() as u64)
        })
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
    let timing = STREAM_REQUEST_DEADLINE
        .try_with(|budget| {
            let state = budget.state.lock().expect("stream deadline lock");
            (
                state.timing.first_byte_ms,
                serde_json::to_value(&state.timing).ok(),
            )
        })
        .unwrap_or_default();
    let payload = serde_json::json!({"error": {"type": error_type, "message": message}});
    let payload_seed = SyncTerminalUsagePayloadSeed {
        report_kind: if error_type == "stream_total_timeout" {
            "local_stream_total_timeout"
        } else {
            "local_stream_first_response_timeout"
        }
        .to_string(),
        status_code: 504,
        response_time_ms: Some(elapsed_ms),
        first_byte_time_ms: timing.0,
        provider_response_headers: None,
        client_response_headers: Some(serde_json::json!({"content-type": "application/json"})),
        provider_response_full: Some(payload.clone()),
        provider_response_body_state: None,
        client_response: Some(payload),
        client_response_body_state: None,
        standardized_usage: None,
        capture_metadata: Some(serde_json::json!({
            "timeout_trigger": error_type,
            "end_to_end_time_ms": elapsed_ms,
            "end_to_end_first_byte_time_ms": timing.1.as_ref()
                .and_then(|timing| timing.get("first_body_elapsed_ms")),
            "stream_timing": timing.1,
        })),
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
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        budget
            .state
            .lock()
            .expect("stream deadline lock")
            .timing
            .first_effective_output_elapsed_ms
            .get_or_insert(budget.started.elapsed().as_millis() as u64);
    });
    if !http_chat_stream_deadline_active() {
        crate::execution_runtime::mark_stream_candidate_watchdog_terminal_started();
    }
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
    let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
        budget.state.lock().expect("stream deadline lock").timing = StreamResponseTiming::default();
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
        let _ = STREAM_REQUEST_DEADLINE.try_with(|budget| {
            let mut state = budget.state.lock().expect("stream deadline lock");
            if !state.pump_owned {
                state.terminal_persistence = false;
                if let Some(active) = budget
                    .active_attempt
                    .lock()
                    .expect("active attempt lock")
                    .as_mut()
                {
                    active.snapshot = None;
                }
                budget.changed.notify_one();
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
        configure_stream_request_deadline(state, plan);
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
    #[tokio::test]
    async fn stream_response_timeout_rechecks_owner_after_polling_generation() {
        for transition in ["handoff", "finished", "persistence"] {
            let started = Instant::now() - Duration::from_secs(1);
            let budget = Arc::new(StreamRequestDeadline {
                started,
                state: Mutex::new(StreamRequestDeadlineState {
                    deadline: started,
                    configured: true,
                    pump_owned: false,
                    terminal_persistence: false,
                    finished: false,
                    timing: StreamResponseTiming::default(),
                }),
                changed: tokio::sync::Notify::new(),
                active_attempt: Mutex::new(None),
                release_resources: Mutex::new(Vec::new()),
                failure_sender: Mutex::new(None),
            });
            let generation_budget = Arc::clone(&budget);
            let mut polls = 0;
            let generation = std::future::poll_fn(move |_| {
                polls += 1;
                if polls == 1 {
                    // select already captured monitor=true. Its first branch
                    // then changes ownership/protection while remaining pending.
                    // No notification is needed for correctness: another owner
                    // may have consumed it before the expired timer is polled.
                    let mut state = generation_budget.state.lock().unwrap();
                    match transition {
                        "handoff" => state.pump_owned = true,
                        "finished" => state.finished = true,
                        "persistence" => state.terminal_persistence = true,
                        _ => unreachable!(),
                    }
                    std::task::Poll::Pending
                } else {
                    std::task::Poll::Ready(Ok(()))
                }
            });
            tokio::time::timeout(
                Duration::from_secs(1),
                run_stream_request_deadline(Arc::clone(&budget), false, generation),
            )
            .await
            .expect("the expired timer must repoll the protected future")
            .unwrap_or_else(|error| panic!("stale owner cancelled {transition}: {error:?}"));
            assert_eq!(
                budget.state.lock().unwrap().finished,
                transition == "finished",
                "the old owner must not claim cancellation for {transition}"
            );
        }
    }

    #[tokio::test]
    async fn stream_response_timeout_request_hands_off_to_pump_before_return() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let pump_finished = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let observed = pump_finished.clone();
        let gate = Arc::new(aether_runtime::ConcurrencyGate::new(
            "stream-timeout-test",
            1,
        ));
        let gate_for_request = gate.clone();
        let (pump, held_body_permit) = with_stream_request_timeout(async move {
            STREAM_REQUEST_DEADLINE.with(|budget| {
                budget.state.lock().unwrap().deadline = Instant::now() + Duration::from_millis(10);
            });
            let held_body_permit =
                hold_stream_deadline_permit(gate_for_request.acquire().await.unwrap());
            let pump = spawn_chat_stream_pump(tx, None, async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                observed.store(true, std::sync::atomic::Ordering::Release);
            });
            // The request future is still alive when the pump deadline expires.
            tokio::time::sleep(Duration::from_millis(30)).await;
            Ok((pump, held_body_permit))
        })
        .await
        .expect("the request owner must not cancel after handoff");
        pump.await.unwrap();
        assert!(rx
            .recv()
            .await
            .unwrap()
            .unwrap_err()
            .to_string()
            .contains(STREAM_TOTAL_TIMEOUT_MESSAGE));
        assert!(!pump_finished.load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(
            gate.snapshot().in_flight,
            0,
            "deadline releases capacity even while the response body still owns its guard"
        );
        drop(held_body_permit);
        assert!(gate.acquire().await.is_ok());
    }

    #[tokio::test]
    async fn stream_response_timeout_terminal_persistence_survives_generation_deadline() {
        let (tx, mut rx) = tokio::sync::mpsc::channel(1);
        let pump = with_stream_request_timeout(async move {
            STREAM_REQUEST_DEADLINE.with(|budget| {
                budget.state.lock().unwrap().deadline = Instant::now() + Duration::from_millis(10);
            });
            Ok(spawn_chat_stream_pump(tx, None, async move {
                finish_stream_request_deadline();
                tokio::time::sleep(Duration::from_millis(100)).await;
            }))
        })
        .await
        .unwrap();
        assert!(tokio::time::timeout(Duration::from_millis(50), rx.recv())
            .await
            .expect("Body EOF must not wait for terminal persistence")
            .is_none());
        assert!(
            !pump.is_finished(),
            "terminal persistence should still be running after Body EOF"
        );
        pump.await
            .expect("terminal persistence survives the same deadline");
    }
}
