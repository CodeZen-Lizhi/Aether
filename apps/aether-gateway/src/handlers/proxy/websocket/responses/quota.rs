//! Quota exhaustion, replay safety, and upstream replacement policy.

use serde_json::Value;
use std::time::Duration;
use uuid::Uuid;
use wreq::ws::message::Message as WreqWsMessage;

use super::adapter::{
    resolve_responses_websocket_adapter, ResponsesWebSocketDrainDirective,
    ResponsesWebSocketRebindSafety,
};
use super::lifecycle::{queue_turn_finalization, PreviousAttemptSettled};
use super::ownership::{
    await_owned_responses_websocket_plan, begin_responses_websocket_turn_with_planned_lease,
    spawn_owned_responses_websocket_plan, OwnedResponsesWebSocketDecision,
};
use super::request::{
    build_planning_parts, planned_response_create_event, ResponsesLiteStaticConfig,
};
use super::state::BoundResponsesConnection;
use super::turn::{prepare_responses_websocket_turn_decision, ResponsesWebSocketTurnOutcome};
use super::upstream::{bind_responses_upstream, close_bound_upstream};
use crate::ai_serving::ResponsesWebSocketPinnedCandidate;
use crate::clock::current_unix_secs;
use crate::handlers::proxy::websocket::ingress::WebSocketRequestContext;
use crate::handlers::proxy::websocket::session::WEBSOCKET_LOG_TRANSPORT;
use crate::handlers::proxy::websocket::transport::close_upstream_socket;
use crate::orchestration::{local_attempt_slot_count, ChatFailureFact};
use crate::AppState;

const LOG_TARGET: &str = "aether_gateway::handlers::proxy::responses_ws";

macro_rules! debug {
    ($($arg:tt)*) => {
        tracing::debug!(target: LOG_TARGET, $($arg)*)
    };
}

macro_rules! warn {
    ($($arg:tt)*) => {
        tracing::warn!(target: LOG_TARGET, $($arg)*)
    };
}

pub(super) async fn detach_exhausted_upstream(
    bound: &mut BoundResponsesConnection,
    directive: ResponsesWebSocketDrainDirective,
    trace_id: &str,
) {
    let exclusion = record_exhausted_bound_key(bound, directive.retry_exclusion_until_unix_secs);
    close_bound_upstream(bound).await;
    // 调用方必须先结束当前 logical turn 再 detach：拆掉上游后 attempt 已经不可能
    // 收到终态，留着它只会等 deadline 或 drop guard 兜底。
    debug_assert!(
        !bound.turn_state.response_in_flight(),
        "an exhausted upstream must be detached after its logical turn ended"
    );
    bound.pending_adapter_drain = None;
    let now_unix_secs = current_unix_secs();
    debug!(
        event_name = "responses_websocket_upstream_detached",
        log_type = "event",
        transport = WEBSOCKET_LOG_TRANSPORT,
        websocket = true,
        trace_id = %trace_id,
        reason = directive.error_code,
        exhausted_key_id = ?exclusion.as_ref().map(|(key_id, _)| key_id),
        retry_exclusion_until_unix_secs = ?exclusion.as_ref().map(|(_, until)| until),
        exhausted_exclusion_count = bound.exhausted_exclusions.len(now_unix_secs),
        "gateway detached an exhausted Responses WebSocket upstream while preserving the client socket"
    );
}

pub(super) fn record_exhausted_bound_key(
    bound: &mut BoundResponsesConnection,
    reset_at_unix_secs: Option<u64>,
) -> Option<(String, u64)> {
    let key_id = bound
        .decision_template
        .key_id
        .as_deref()
        .map(str::trim)
        .filter(|key_id| !key_id.is_empty())?
        .to_string();
    let provider_account_id = bound
        .adapter
        .exhaustion_exclusion_identity(&bound.decision_template)
        .and_then(|identity| identity.account_id);
    let exclusion_until = bound.exhausted_exclusions.exclude(
        key_id.clone(),
        provider_account_id,
        reset_at_unix_secs,
        current_unix_secs(),
    );
    Some((key_id, exclusion_until))
}

/// 为同一个 logical turn 规划并绑定下一个 attempt。
///
/// `_previous_settled` 不被使用，它只是把「上一个 attempt 已经结算完毕」这个
/// 前置条件写进签名：规划要读 health / adaptive / pool 状态，而这些是上一个
/// attempt 结算时才投射的；它的 pool key lease 也要先释放，否则替代 key 的挑选
/// 会看到一把仍被占用的 key。
pub(super) async fn retry_active_turn_after_quota_exhaustion(
    bound: &mut BoundResponsesConnection,
    state: &AppState,
    context: &WebSocketRequestContext,
    _previous_settled: PreviousAttemptSettled,
    failure: Option<(ChatFailureFact, Option<u64>)>,
) -> bool {
    let Some(deadline) = bound
        .turn_state
        .logical()
        .and_then(|turn| turn.first_output_deadline)
    else {
        return false;
    };
    tokio::time::timeout_at(
        deadline.into(),
        retry_active_turn_inner(bound, state, context, failure),
    )
    .await
    .unwrap_or(false)
}

async fn retry_active_turn_inner(
    bound: &mut BoundResponsesConnection,
    state: &AppState,
    context: &WebSocketRequestContext,
    failure: Option<(ChatFailureFact, Option<u64>)>,
) -> bool {
    // `LogicalTurn::client_event` is intentionally redacted before it is
    // retained for replay.  The binding, however, keeps the hash of the raw
    // client-side Responses Lite tools/instructions so a later continuation
    // can compare the client's plaintext configuration before redaction.
    // Preserve that chain identity across this transparent rebind instead of
    // replacing it with the hash `bind_responses_upstream` derives from the
    // redacted replay event.
    let responses_lite_static_config = bound.responses_lite_static_config.clone();
    let Some(active) = bound.turn_state.logical_mut() else {
        return false;
    };
    if let Some(reason) = active.retry_block_reason() {
        debug!(
            event_name = "responses_websocket_retry_skipped",
            log_type = "event",
            transport = WEBSOCKET_LOG_TRANSPORT,
            websocket = true,
            trace_id = %context.trace_id,
            turn_index = active.turn_index,
            logical_turn_id = %active.logical_turn_id,
            turn_attempt = active.turn_attempt,
            reason,
            "gateway will not transparently replay an unsafe Responses WebSocket turn"
        );
        return false;
    }
    active.turn_attempt = active.turn_attempt.saturating_add(1);
    let client_event = active.client_event.clone();
    let Some(turn_control) = active.turn_control.clone() else {
        warn!(
            event_name = "responses_websocket_quota_retry_control_missing",
            log_type = "ops",
            transport = WEBSOCKET_LOG_TRANSPORT,
            websocket = true,
            trace_id = %context.trace_id,
            "gateway refused to retry a WebSocket turn without its live authorization snapshot"
        );
        return false;
    };
    let turn_index = active.turn_index;
    let logical_turn_id = active.logical_turn_id.clone();
    let turn_attempt = active.turn_attempt;

    let retry_exclusion_until_unix_secs = bound
        .pending_adapter_drain
        .and_then(|directive| directive.retry_exclusion_until_unix_secs);
    let exhausted_key_id = if failure.is_none() {
        record_exhausted_bound_key(bound, retry_exclusion_until_unix_secs).map(|(key, _)| key)
    } else {
        None
    };

    let Some(current_key) = bound.decision_template.key_id.clone() else {
        return false;
    };
    let max_attempts = match (
        bound.decision_template.provider_id.as_deref(),
        bound.decision_template.endpoint_id.as_deref(),
    ) {
        (Some(provider), Some(endpoint)) => match state
            .read_provider_transport_snapshot_arc(provider, endpoint, &current_key)
            .await
        {
            Ok(Some(snapshot)) => local_attempt_slot_count(&snapshot),
            _ => return false,
        },
        _ => return false,
    };
    let cooldown_wait = match state
        .read_provider_catalog_keys_by_ids_strong(std::slice::from_ref(&current_key))
        .await
    {
        Ok(keys) => keys
            .into_iter()
            .next()
            .and_then(|key| {
                aether_scheduler_core::provider_key_rate_limit_cooldown(
                    &key,
                    bound
                        .decision_template
                        .provider_api_format
                        .as_deref()
                        .unwrap_or("openai:responses"),
                )
            })
            .filter(|cooldown| {
                cooldown.until_unix_secs.saturating_mul(1000) > crate::clock::current_unix_ms()
            })
            .map(|cooldown| {
                Duration::from_millis(
                    cooldown
                        .until_unix_secs
                        .saturating_mul(1000)
                        .saturating_sub(crate::clock::current_unix_ms()),
                )
            }),
        Err(_) => return false,
    };
    let active = bound
        .turn_state
        .logical_mut()
        .expect("retry retains logical turn");
    let attempts = *active
        .attempts_by_key
        .entry(current_key.clone())
        .or_insert(1);
    let retry_after = failure.and_then(|(_, retry_after)| retry_after);
    let delay = cooldown_wait
        .or_else(|| retry_after.map(Duration::from_secs))
        .unwrap_or_else(|| {
            if failure.is_some_and(|(fact, _)| fact.rate_limited) {
                Duration::from_secs(1u64 << attempts.saturating_sub(1).min(6))
            } else {
                let jitter_ms = 500 + (Uuid::new_v4().as_u128() % 501) as u64;
                Duration::from_millis((jitter_ms << attempts.saturating_sub(1).min(2)).min(2_000))
            }
        });
    let retry_same_key = failure.is_some_and(|(fact, _)| fact.retryable())
        && active.reserve_same_key_retry(&current_key, max_attempts, delay);
    if retry_same_key {
        tokio::time::sleep(delay).await;
    } else {
        active.excluded_keys.insert(current_key.clone());
    }

    let turn_request_id = Uuid::new_v4().to_string();
    let now_unix_secs = current_unix_secs();
    let mut excluded_key_ids = bound.exhausted_exclusions.key_ids(now_unix_secs);
    excluded_key_ids.extend(
        bound
            .turn_state
            .logical()
            .unwrap()
            .excluded_keys
            .iter()
            .cloned(),
    );
    let excluded_codex_account_ids = bound.exhausted_exclusions.codex_account_ids(now_unix_secs);
    let excluded_codex_account_ids =
        (!excluded_codex_account_ids.is_empty()).then_some(excluded_codex_account_ids);
    let mut pinned = retry_same_key
        .then(|| ResponsesWebSocketPinnedCandidate::from_decision(&bound.decision_template))
        .flatten();
    let planned = loop {
        let result = await_owned_responses_websocket_plan(
            spawn_owned_responses_websocket_plan(
                state.clone(),
                build_planning_parts(context),
                turn_request_id.clone(),
                turn_control.decision.clone(),
                turn_control.auth_snapshot.clone(),
                client_event.clone(),
                (!excluded_key_ids.is_empty()).then_some(excluded_key_ids.clone()),
                excluded_codex_account_ids.clone(),
                pinned.clone(),
            ),
            bound
                .turn_state
                .logical()
                .and_then(|turn| turn.first_output_deadline)
                .expect("retry requires first output budget"),
        )
        .await;
        match result {
            Ok(Some(decision)) => break decision,
            Ok(None) if pinned.take().is_some() => {
                excluded_key_ids.insert(current_key.clone());
                bound
                    .turn_state
                    .logical_mut()
                    .unwrap()
                    .excluded_keys
                    .insert(current_key.clone());
                continue;
            }
            Ok(None) => {
                warn!(
                    event_name = "responses_websocket_retry_provider_unavailable",
                    log_type = "ops",
                    transport = WEBSOCKET_LOG_TRANSPORT,
                    websocket = true,
                    trace_id = %context.trace_id,
                    exhausted_key_id = ?exhausted_key_id,
                    "gateway could not find an eligible Responses WebSocket retry target"
                );
                return false;
            }
            Err(error) => {
                warn!(
                    event_name = "responses_websocket_retry_planning_failed",
                    log_type = "ops",
                    transport = WEBSOCKET_LOG_TRANSPORT,
                    websocket = true,
                    trace_id = %context.trace_id,
                    exhausted_key_id = ?exhausted_key_id,
                    error = ?error,
                    "gateway could not plan an eligible Responses WebSocket retry target"
                );
                return false;
            }
        }
    };
    let OwnedResponsesWebSocketDecision {
        planned,
        planning_parts,
        planned_lease,
    } = planned;
    let adapter = resolve_responses_websocket_adapter(planned.adapter);
    let normalization = planned.normalization;
    let decision = planned.execution;
    if decision
        .key_id
        .as_ref()
        .is_some_and(|key| excluded_key_ids.contains(key))
    {
        planned_lease.release().await;
        warn!(
            event_name = "responses_websocket_quota_retry_selected_exhausted_key",
            log_type = "ops",
            transport = WEBSOCKET_LOG_TRANSPORT,
            websocket = true,
            trace_id = %context.trace_id,
            key_id = ?decision.key_id,
            "gateway rejected an alternate Responses WebSocket plan that reused the exhausted key"
        );
        return false;
    }
    let provider_event = match planned_response_create_event(
        &decision,
        &normalization,
        &client_event,
    )
    .and_then(|event| {
        serde_json::from_str::<Value>(&event).map_err(|_| "response_create_serialization_failed")
    }) {
        Ok(event) => event,
        Err(code) => {
            planned_lease.release().await;
            warn!(
                event_name = "responses_websocket_quota_retry_normalization_failed",
                log_type = "ops",
                transport = WEBSOCKET_LOG_TRANSPORT,
                websocket = true,
                trace_id = %context.trace_id,
                error_code = code,
                "gateway could not rebuild a Responses response.create for transparent quota retry"
            );
            return false;
        }
    };
    let replacement_provider_store = provider_event.get("store") == Some(&Value::Bool(true));
    let turn_decision = prepare_responses_websocket_turn_decision(
        &decision,
        turn_request_id,
        true,
        &client_event,
        &provider_event,
        &context.trace_id,
        turn_index,
        &logical_turn_id,
        turn_attempt,
    );
    let mut turn = match begin_responses_websocket_turn_with_planned_lease(
        state,
        &context.trace_id,
        planning_parts,
        &turn_control.decision,
        turn_decision,
        &client_event,
        planned_lease,
        bound
            .turn_state
            .logical()
            .and_then(|turn| turn.first_output_deadline)
            .expect("retry requires first output budget"),
    )
    .await
    {
        Ok(turn) => turn,
        Err(error) => {
            warn!(
                event_name = "responses_websocket_quota_retry_reporting_unavailable",
                log_type = "ops",
                transport = WEBSOCKET_LOG_TRANSPORT,
                websocket = true,
                trace_id = %context.trace_id,
                error = ?error,
                "gateway could not start usage and audit tracking for transparent quota retry"
            );
            return false;
        }
    };
    let mut replacement = match turn
        .with_probe_lease(bind_responses_upstream(
            &decision,
            normalization,
            &client_event,
            adapter,
        ))
        .await
    {
        Ok(connection) => connection,
        Err(code) => {
            queue_turn_finalization(
                bound,
                state,
                turn,
                ResponsesWebSocketTurnOutcome::upstream_connect_failed(code),
            )
            .await;
            warn!(
                event_name = "responses_websocket_retry_rebind_failed",
                log_type = "ops",
                transport = WEBSOCKET_LOG_TRANSPORT,
                websocket = true,
                trace_id = %context.trace_id,
                error_code = code,
                "gateway could not bind the Responses WebSocket retry target"
            );
            return false;
        }
    };

    turn.mark_upstream_request_sent();
    turn.set_provider_response_headers(replacement.upstream_response_headers.clone());
    let replacement_upstream = replacement
        .upstream
        .take()
        .expect("newly bound Responses upstream should be present");
    if let Some(mut previous_upstream) = bound.upstream.replace(replacement_upstream) {
        close_upstream_socket(&mut previous_upstream, None).await;
    }
    // The replacement socket has no access to response IDs cached only on
    // the exhausted physical connection. Continuation turns are never quota
    // replayed, so clearing here cannot discard the active turn's parent.
    bound.continuation_response_ids.clear();
    let previous_key_id = bound.decision_template.key_id.clone();
    bound.adapter = replacement.adapter;
    bound.client_model = replacement.client_model;
    bound.provider_model = replacement.provider_model;
    bound.decision_template = replacement.decision_template;
    bound.body_normalization = replacement.body_normalization;
    bound.responses_lite_static_config = responses_lite_static_config_after_rebind(
        responses_lite_static_config,
        replacement.responses_lite_static_config,
    );
    bound.binding_identity = replacement.binding_identity;
    // 同一个 logical turn 的下一个 attempt 就位。状态不符时把 attempt 交回
    // drop guard 结算并让调用方走「透明重试失败」分支，不静默丢弃一条已经写了
    // pending usage 行、占着 candidate 和 pool key lease 的 attempt。
    if let Err(orphan) = bound.turn_state.resume(turn) {
        drop(orphan);
        return false;
    }
    if let Some(logical) = bound.turn_state.logical_mut() {
        logical.provider_store = replacement_provider_store;
        if let Some(key) = bound.decision_template.key_id.as_ref() {
            logical.record_attempt(key);
        }
    }
    bound.upstream_response_headers = replacement.upstream_response_headers;
    bound.pending_adapter_drain = None;
    debug!(
        event_name = "responses_websocket_retry_rebound",
        log_type = "event",
        transport = WEBSOCKET_LOG_TRANSPORT,
        websocket = true,
        trace_id = %context.trace_id,
        turn_index,
        logical_turn_id = %logical_turn_id,
        turn_attempt,
        previous_key_id = ?previous_key_id,
        key_id = ?bound.decision_template.key_id,
        "gateway transparently rebound a rejected Responses WebSocket turn"
    );
    true
}

fn responses_lite_static_config_after_rebind(
    previous: Option<ResponsesLiteStaticConfig>,
    replacement: Option<ResponsesLiteStaticConfig>,
) -> Option<ResponsesLiteStaticConfig> {
    replacement.map(|replacement| previous.unwrap_or(replacement))
}

pub(super) fn is_usage_limit_error_event(event: &Value) -> bool {
    let is_error = |value: &Value| {
        value.get("type").and_then(Value::as_str) == Some("error")
            && value.pointer("/error/type").and_then(Value::as_str) == Some("usage_limit_reached")
    };
    is_error(event)
        || event
            .get("chunks")
            .and_then(Value::as_array)
            .is_some_and(|chunks| chunks.iter().any(is_error))
}

pub(super) fn observe_active_response_rebind_safety(
    bound: &mut BoundResponsesConnection,
    event: &Value,
) {
    let ResponsesWebSocketRebindSafety::Unsafe { reason } =
        bound.adapter.rebind_safety_for_upstream_event(event)
    else {
        return;
    };
    if let Some(active) = bound.turn_state.logical_mut() {
        active.mark_retry_unsafe(reason);
    }
}

pub(super) fn mark_active_response_retry_unsafe(
    bound: &mut BoundResponsesConnection,
    reason: &'static str,
) {
    if let Some(active) = bound.turn_state.logical_mut() {
        active.mark_retry_unsafe(reason);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::responses_lite_static_config_after_rebind;
    use crate::handlers::proxy::websocket::responses::request::{
        prepare_responses_lite_continuation, ResponsesLiteStaticConfig,
    };

    #[test]
    fn quota_rebind_preserves_the_raw_responses_lite_static_hash() {
        let raw = json!({
            "type": "response.create",
            "model": "gpt-5.6-sol",
            "instructions": "Contact alice@example.com before using the tool",
            "tools": [{
                "type": "function",
                "name": "lookup",
                "description": "Look up alice@example.com",
                "parameters": {"type": "object", "properties": {}}
            }],
            "input": [{"role": "user", "content": "hello"}]
        });
        let redacted_replay = json!({
            "type": "response.create",
            "model": "gpt-5.6-sol",
            "instructions": "Contact <EMAIL_2> before using the tool",
            "tools": [{
                "type": "function",
                "name": "lookup",
                "description": "Look up <EMAIL_1>",
                "parameters": {"type": "object", "properties": {}}
            }],
            "input": [{"role": "user", "content": "hello"}]
        });
        let raw_static_config = ResponsesLiteStaticConfig::from_response_create(&raw);
        let redacted_static_config =
            ResponsesLiteStaticConfig::from_response_create(&redacted_replay);
        assert_ne!(raw_static_config, redacted_static_config);

        let rebound_static_config = responses_lite_static_config_after_rebind(
            Some(raw_static_config.clone()),
            Some(redacted_static_config),
        )
        .expect("the replacement still uses Responses Lite");
        assert_eq!(rebound_static_config, raw_static_config);

        let continuation = json!({
            "type": "response.create",
            "model": "gpt-5.6-sol",
            "previous_response_id": "resp_after_quota_retry",
            "instructions": "Contact alice@example.com before using the tool",
            "tools": [{
                "type": "function",
                "name": "lookup",
                "description": "Look up alice@example.com",
                "parameters": {"type": "object", "properties": {}}
            }],
            "input": [{
                "type": "function_call_output",
                "call_id": "call_1",
                "output": "ok"
            }]
        });
        let prepared = prepare_responses_lite_continuation(&continuation, &rebound_static_config)
            .expect("an unchanged plaintext continuation must survive a quota retry");
        assert!(prepared.get("tools").is_none());
        assert!(prepared.get("instructions").is_none());
    }

    #[test]
    fn quota_rebind_still_tracks_the_replacement_contract() {
        let raw = ResponsesLiteStaticConfig::from_response_create(&json!({
            "type": "response.create",
            "tools": [{"type": "function", "name": "lookup", "parameters": {}}]
        }));
        let replacement = ResponsesLiteStaticConfig::from_response_create(&json!({
            "type": "response.create",
            "instructions": "replacement"
        }));

        assert_eq!(
            responses_lite_static_config_after_rebind(Some(raw), None),
            None,
            "a non-Lite replacement must clear the Lite chain marker"
        );
        assert_eq!(
            responses_lite_static_config_after_rebind(None, Some(replacement.clone())),
            Some(replacement),
            "a newly selected Lite contract has no earlier raw hash to preserve"
        );
    }
}
