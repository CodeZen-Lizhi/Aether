use std::time::Duration;

use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogKeyHealthPendingFact, ProviderCatalogKeyHealthSettlement,
    ProviderCatalogKeyHealthSettlementResult, ProviderCatalogKeyHealthStateUpdate,
    ProviderCatalogKeyOAuthCredentialFence, StoredProviderCatalogKey,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tracing::warn;

use super::effects::{chat_credential_fingerprint, ChatHealthAttemptFence};
use super::{ChatFailureFact, ChatHealthPenalty, PROBE_LEASES_REPORT_FIELD};
use crate::clock::current_unix_secs;
use crate::{AppState, GatewayError};

const FACT_TTL_SECS: u64 = 24 * 60 * 60;
const SETTLE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Clone, Serialize, Deserialize)]
struct StoredFailure {
    penalty: ChatHealthPenalty,
    rate_limited: bool,
    reason: String,
    retry_after_secs: Option<u64>,
}

impl StoredFailure {
    fn fact(&self) -> Option<ChatFailureFact> {
        let reason = match self.reason.as_str() {
            "neutral" => "neutral",
            "upstream_timeout" => "upstream_timeout",
            "upstream_transport" => "upstream_transport",
            "upstream_protocol" => "upstream_protocol",
            "credential_unavailable" => "credential_unavailable",
            "upstream_busy" => "upstream_busy",
            "rate_limited" => "rate_limited",
            "upstream_temporary" => "upstream_temporary",
            "unconfirmed_authentication" => "unconfirmed_authentication",
            "upstream_failure" => "upstream_failure",
            "invalid_success" => "invalid_success",
            "unclassified" => "unclassified",
            _ => return None,
        };
        match self.penalty {
            ChatHealthPenalty::Points(1..=3)
            | ChatHealthPenalty::Unavailable
            | ChatHealthPenalty::Neutral => Some(ChatFailureFact {
                penalty: self.penalty,
                rate_limited: self.rate_limited,
                reason,
            }),
            _ => None,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
struct TerminalFact {
    provider_id: String,
    fence: ChatHealthAttemptFence,
    probe_leases: Value,
    failure: Option<StoredFailure>,
}

pub(super) fn build_pending_fact(
    provider_id: &str,
    key_id: &str,
    api_format: &str,
    fence: ChatHealthAttemptFence,
    report_context: Option<&Value>,
    failure: Option<(ChatFailureFact, Option<u64>)>,
    observed_at_unix_secs: u64,
) -> ProviderCatalogKeyHealthPendingFact {
    let terminal = TerminalFact {
        provider_id: provider_id.to_owned(),
        fence: fence.clone(),
        probe_leases: report_context
            .and_then(|report| report.get(PROBE_LEASES_REPORT_FIELD))
            .cloned()
            .unwrap_or_else(|| json!([])),
        failure: failure.map(|(fact, retry_after_secs)| StoredFailure {
            penalty: fact.penalty,
            rate_limited: fact.rate_limited,
            reason: fact.reason.to_owned(),
            retry_after_secs,
        }),
    };
    ProviderCatalogKeyHealthPendingFact {
        attempt_id: fence.attempt_id,
        key_id: key_id.to_owned(),
        api_format: api_format.to_owned(),
        policy_version: super::chat_health::CHAT_HEALTH_POLICY_VERSION as u32,
        attempt_started_at_unix_secs: fence.started_at_unix_secs,
        observed_at_unix_secs,
        fact: serde_json::to_value(terminal).expect("terminal fact is serializable"),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettlementProgress {
    Applied,
    AlreadyApplied,
    Obsolete,
    Pending,
}

fn probe_expiry(key: &StoredProviderCatalogKey, format: &str) -> Option<u64> {
    [
        (
            key.circuit_breaker_by_format.as_ref(),
            "half_open_lease",
            "half_open_until_unix_secs",
        ),
        (
            key.health_by_format.as_ref(),
            "rate_limit_probe_lease",
            "rate_limit_probe_until_unix_secs",
        ),
    ]
    .into_iter()
    .filter_map(|(state, owner, deadline)| {
        let payload = state?.get(format)?;
        payload.get(owner).filter(|v| !v.is_null())?;
        payload.get(deadline)?.as_u64()
    })
    .min()
}

async fn settle_once(
    state: &AppState,
    pending: &ProviderCatalogKeyHealthPendingFact,
) -> Result<SettlementProgress, GatewayError> {
    let lock = super::effects::chat_health_effect_lock(&pending.key_id);
    let _guard = lock.lock().await;
    if state
        .provider_catalog_key_health_attempt_is_settled(pending)
        .await?
    {
        return Ok(SettlementProgress::AlreadyApplied);
    }
    if current_unix_secs()
        >= pending
            .attempt_started_at_unix_secs
            .saturating_add(FACT_TTL_SECS)
        || pending.policy_version != super::chat_health::CHAT_HEALTH_POLICY_VERSION as u32
    {
        return Ok(SettlementProgress::Obsolete);
    }
    let terminal: TerminalFact = match serde_json::from_value(pending.fact.clone()) {
        Ok(fact) => fact,
        Err(_) => {
            warn!(event_name = "chat_health_pending_fact_invalid", attempt_id = %pending.attempt_id);
            return Ok(SettlementProgress::Obsolete);
        }
    };
    if terminal.fence.attempt_id != pending.attempt_id
        || terminal.fence.started_at_unix_secs != pending.attempt_started_at_unix_secs
    {
        return Ok(SettlementProgress::Obsolete);
    }
    let Some(provider) = state
        .read_provider_catalog_providers_by_ids(std::slice::from_ref(&terminal.provider_id))
        .await?
        .pop()
    else {
        return Ok(SettlementProgress::Obsolete);
    };
    let report = json!({PROBE_LEASES_REPORT_FIELD: terminal.probe_leases});
    for _ in 0..4 {
        let Some(key) = state
            .read_provider_catalog_keys_by_ids_strong(std::slice::from_ref(&pending.key_id))
            .await?
            .pop()
        else {
            return Ok(SettlementProgress::Obsolete);
        };
        let epoch = key
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|v| v.get(&pending.api_format))
            .and_then(|v| v.get("circuit_epoch"))
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if key.provider_id != terminal.provider_id
            || terminal.fence.credential_fingerprint
                != chat_credential_fingerprint(&key, &provider.provider_type)
            || terminal.fence.circuit_epoch != epoch
            || !super::effects::chat_probe_reports_are_current(
                &key,
                &pending.api_format,
                Some(&report),
                current_unix_secs(),
            )
        {
            return Ok(SettlementProgress::Obsolete);
        }
        let projection = match terminal.failure.as_ref() {
            Some(failure) => {
                let Some(fact) = failure.fact() else {
                    return Ok(SettlementProgress::Obsolete);
                };
                super::chat_health::project_chat_failure(
                    key.health_by_format.as_ref(),
                    key.circuit_breaker_by_format.as_ref(),
                    &pending.api_format,
                    fact,
                    pending.observed_at_unix_secs,
                    failure.retry_after_secs,
                    key.max_probe_interval_minutes,
                )
            }
            None => super::chat_health::project_chat_success(
                key.health_by_format.as_ref(),
                key.circuit_breaker_by_format.as_ref(),
                &pending.api_format,
                pending.observed_at_unix_secs,
            ),
        };
        let Some(projection) = projection else {
            return Ok(SettlementProgress::Obsolete);
        };
        let settlement = ProviderCatalogKeyHealthSettlement {
            attempt_id: pending.attempt_id.clone(),
            api_format: pending.api_format.clone(),
            policy_version: pending.policy_version,
            attempt_started_at_unix_secs: pending.attempt_started_at_unix_secs,
            expected_credential: ProviderCatalogKeyOAuthCredentialFence {
                encrypted_api_key: key.encrypted_api_key.clone(),
                auth_type: key.auth_type.clone(),
                provider_id: key.provider_id.clone(),
                provider_type: provider.provider_type.clone(),
            },
            expected_encrypted_auth_config: key.encrypted_auth_config.clone(),
            expected_circuit_epoch: epoch,
            expected_probe_lease_expires_at_unix_secs: probe_expiry(&key, &pending.api_format),
            update: ProviderCatalogKeyHealthStateUpdate {
                key_id: pending.key_id.clone(),
                expected_encrypted_auth_config: key.encrypted_auth_config,
                expected_health_by_format: key.health_by_format,
                expected_circuit_breaker_by_format: key.circuit_breaker_by_format,
                health_by_format: Some(projection.health_by_format),
                circuit_breaker_by_format: Some(projection.circuit_breaker_by_format),
            },
        };
        match state
            .settle_provider_catalog_key_health_attempt(&settlement)
            .await?
        {
            ProviderCatalogKeyHealthSettlementResult::Applied => {
                return Ok(SettlementProgress::Applied)
            }
            ProviderCatalogKeyHealthSettlementResult::Duplicate => {
                return Ok(SettlementProgress::AlreadyApplied)
            }
            ProviderCatalogKeyHealthSettlementResult::Conflict => tokio::task::yield_now().await,
            _ => return Ok(SettlementProgress::Obsolete),
        }
    }
    Ok(SettlementProgress::Pending)
}

async fn process_pending(state: &AppState, pending: &ProviderCatalogKeyHealthPendingFact) -> bool {
    let progress = match tokio::time::timeout(SETTLE_TIMEOUT, settle_once(state, pending)).await {
        Ok(Ok(progress)) => progress,
        Ok(Err(error)) => {
            warn!(event_name = "chat_health_settlement_deferred", attempt_id = %pending.attempt_id, ?error);
            return false;
        }
        Err(_) => {
            warn!(event_name = "chat_health_settlement_timed_out", attempt_id = %pending.attempt_id);
            return false;
        }
    };
    if progress == SettlementProgress::Obsolete {
        warn!(event_name = "chat_health_fact_obsolete", attempt_id = %pending.attempt_id,
            key_id = %pending.key_id, api_format = %pending.api_format,
            "terminal fact no longer has a valid credential, circuit or probe generation");
    }
    if progress != SettlementProgress::Pending {
        match tokio::time::timeout(
            SETTLE_TIMEOUT,
            state.remove_provider_catalog_key_health_pending_fact(pending),
        )
        .await
        {
            Ok(Ok(())) => {}
            Ok(Err(error)) => {
                warn!(event_name = "chat_health_pending_cleanup_failed", attempt_id = %pending.attempt_id, ?error);
            }
            Err(_) => {
                warn!(event_name = "chat_health_pending_cleanup_timed_out", attempt_id = %pending.attempt_id);
            }
        }
    }
    matches!(
        progress,
        SettlementProgress::Applied | SettlementProgress::AlreadyApplied
    )
}

/// Start the durable handoff before the caller can cancel its effect-stage wait.
pub(super) async fn persist_and_settle(
    state: &AppState,
    pending: ProviderCatalogKeyHealthPendingFact,
) -> bool {
    let state = state.clone();
    let worker = tokio::spawn(async move {
        let retained = tokio::time::timeout(
            SETTLE_TIMEOUT,
            state.enqueue_provider_catalog_key_health_fact(&pending),
        )
        .await;
        match retained {
            Ok(Ok(canonical)) => process_pending(&state, &canonical).await,
            _ => {
                // The enqueue may have committed before an uncertain failure.
                // Without its authoritative payload, never settle this delivery:
                // an earlier pending fact may have a different time or outcome.
                // Retain only the redacted DTO in diagnostics for recovery.
                warn!(event_name = "chat_health_pending_retention_failed", attempt_id = %pending.attempt_id);
                warn!(event_name = "chat_health_fact_not_persisted", fact = %serde_json::to_string(&pending).unwrap_or_default());
                false
            }
        }
    });
    worker.await.unwrap_or(false)
}

pub(crate) async fn drain_pending_health_facts(state: &AppState) -> Result<usize, GatewayError> {
    let pending = state
        .list_provider_catalog_key_health_pending_facts(32)
        .await?;
    let count = pending.len();
    for fact in pending {
        process_pending(state, &fact).await;
    }
    Ok(count)
}

pub(crate) fn spawn_health_settlement_worker(app: AppState) -> Option<tokio::task::JoinHandle<()>> {
    if !app.has_provider_catalog_data_reader() || !app.has_provider_catalog_data_writer() {
        return None;
    }
    Some(crate::task_runtime::spawn_singleton_worker(
        app,
        crate::task_runtime::TASK_KEY_CHAT_HEALTH_SETTLEMENT,
        |app| async move {
            loop {
                if let Err(error) = drain_pending_health_facts(&app).await {
                    warn!(event_name = "chat_health_pending_drain_failed", ?error);
                }
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        },
    ))
}
