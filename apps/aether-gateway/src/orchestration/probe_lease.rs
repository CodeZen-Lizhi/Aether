use std::time::Duration;

use aether_data_contracts::repository::provider_catalog::{
    ProviderCatalogKeyHealthSettlement, ProviderCatalogKeyHealthSettlementResult,
    ProviderCatalogKeyHealthStateUpdate, ProviderCatalogKeyOAuthCredentialFence,
    StoredProviderCatalogKey,
};
use aether_scheduler_core::{is_provider_key_circuit_open, provider_key_rate_limit_cooldown};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::sync::{oneshot, watch};
use tokio::task::JoinHandle;
use tracing::warn;

use super::health::{
    project_local_key_circuit_probe_reservation, project_local_rate_limit_probe_reservation,
};
use crate::clock::current_unix_secs;
use crate::{AppState, GatewayError};

const LEASE_TTL_SECS: u64 = 60;
const RENEW_INTERVAL: Duration = Duration::from_secs(20);
const OPERATION_TIMEOUT: Duration = Duration::from_secs(5);
const CAS_ATTEMPTS: usize = 4;

pub(crate) const PROBE_LEASES_REPORT_FIELD: &str = "probe_leases";

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProbeLeaseKind {
    Circuit,
    RateLimit,
}

impl ProbeLeaseKind {
    fn owner_field(self) -> &'static str {
        match self {
            Self::Circuit => "half_open_lease",
            Self::RateLimit => "rate_limit_probe_lease",
        }
    }

    fn deadline_field(self) -> &'static str {
        match self {
            Self::Circuit => "half_open_until_unix_secs",
            Self::RateLimit => "rate_limit_probe_until_unix_secs",
        }
    }

    fn state(self, key: &StoredProviderCatalogKey) -> Option<&Value> {
        match self {
            Self::Circuit => key.circuit_breaker_by_format.as_ref(),
            Self::RateLimit => key.health_by_format.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeLeaseLoss {
    Expired,
    RenewalDeadline,
    OwnershipChanged,
    StorageUnavailable,
    Contention,
    WorkerStopped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProbeLeaseStatus {
    Active { until_unix_secs: u64 },
    Lost(ProbeLeaseLoss),
    Released,
}

#[derive(Debug)]
pub(crate) enum LocalProbeLeaseClaim {
    NotRequired,
    Acquired(LocalProbeLeaseGuard),
    Unavailable,
}

/// This report is safe to attach to terminal attempt context. Ciphertexts stay
/// exclusively in the private worker's credential fence.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
struct ProbeLeaseReport {
    owner_token: String,
    kind: ProbeLeaseKind,
    key_id: String,
    api_format: String,
    credential_fingerprint: String,
    circuit_epoch: u64,
    source: Value,
}

impl ProbeLeaseReport {
    fn payload<'a>(&self, key: &'a StoredProviderCatalogKey) -> Option<&'a Value> {
        self.kind.state(key)?.get(&self.api_format)
    }

    fn matches(&self, key: &StoredProviderCatalogKey) -> bool {
        self.same_owner_generation(key)
            && self.source == source_fence(key, &self.api_format, self.kind)
    }

    fn same_owner_generation(&self, key: &StoredProviderCatalogKey) -> bool {
        self.key_id == key.id
            && self.credential_fingerprint == credential_fingerprint(key)
            && circuit_epoch(key, &self.api_format) == Some(self.circuit_epoch)
            && self
                .payload(key)
                .and_then(|v| v.get(self.kind.owner_field()))
                == Some(&serde_json::to_value(self).expect("lease report is serializable"))
    }

    fn expires_at(&self, key: &StoredProviderCatalogKey) -> Option<u64> {
        self.payload(key)?.get(self.kind.deadline_field())?.as_u64()
    }
}

#[derive(Clone)]
struct LeaseIdentity {
    report: ProbeLeaseReport,
    credential: ProviderCatalogKeyOAuthCredentialFence,
    encrypted_auth_config: Option<String>,
}

#[derive(Debug)]
pub(crate) struct LocalProbeLeaseGuard {
    report: ProbeLeaseReport,
    stop: Option<oneshot::Sender<()>>,
    status: watch::Receiver<ProbeLeaseStatus>,
    worker: Option<JoinHandle<()>>,
}

impl LocalProbeLeaseGuard {
    fn start(state: AppState, identity: LeaseIdentity, until_unix_secs: u64) -> Self {
        let (stop, stopped) = oneshot::channel();
        let (status_tx, status) = watch::channel(ProbeLeaseStatus::Active { until_unix_secs });
        let report = identity.report.clone();
        let worker = tokio::spawn(run_lease_worker(state, identity, stopped, status_tx));
        Self {
            report,
            stop: Some(stop),
            status,
            worker: Some(worker),
        }
    }

    pub(crate) fn status(&self) -> ProbeLeaseStatus {
        match *self.status.borrow() {
            ProbeLeaseStatus::Active { until_unix_secs }
                if current_unix_secs() >= until_unix_secs =>
            {
                ProbeLeaseStatus::Lost(ProbeLeaseLoss::Expired)
            }
            status => status,
        }
    }

    pub(crate) fn report_context(&self) -> Value {
        serde_json::to_value(&self.report).expect("lease report is serializable")
    }

    /// Select this future alongside execution. Dropping this future is safe;
    /// dropping the guard cancels renewal and releases only this owner.
    pub(crate) async fn lost(&mut self) -> ProbeLeaseLoss {
        loop {
            match self.status() {
                ProbeLeaseStatus::Lost(reason) => return reason,
                ProbeLeaseStatus::Released => return std::future::pending().await,
                ProbeLeaseStatus::Active { until_unix_secs } => {
                    let remaining = until_unix_secs.saturating_sub(current_unix_secs());
                    tokio::select! {
                        changed = self.status.changed() => {
                            if changed.is_err() { return ProbeLeaseLoss::WorkerStopped; }
                        }
                        _ = tokio::time::sleep(Duration::from_secs(remaining)) => {}
                    }
                }
            }
        }
    }

    /// Call after terminal health settlement; it must not replace a known
    /// upstream result with a lease failure caused by that same settlement.
    pub(crate) async fn finish(mut self) -> ProbeLeaseStatus {
        self.signal_stop();
        if let Some(worker) = self.worker.take() {
            if worker.await.is_err() {
                return ProbeLeaseStatus::Lost(ProbeLeaseLoss::WorkerStopped);
            }
        }
        self.status()
    }

    fn signal_stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for LocalProbeLeaseGuard {
    fn drop(&mut self) {
        self.signal_stop();
        // The task already exists and owns a bounded release operation. Dropping
        // its JoinHandle detaches it, so cancellation cannot skip cleanup.
    }
}

pub(crate) fn probe_lease_report_is_current(
    key: &StoredProviderCatalogKey,
    report: &Value,
    now: u64,
) -> bool {
    serde_json::from_value::<ProbeLeaseReport>(report.clone())
        .ok()
        .is_some_and(|report| {
            report.matches(key) && report.expires_at(key).is_some_and(|expires| now < expires)
        })
}

fn credential_fingerprint(key: &StoredProviderCatalogKey) -> String {
    let bytes = serde_json::to_vec(&(
        &key.encrypted_api_key,
        &key.encrypted_auth_config,
        &key.auth_type,
        &key.provider_id,
    ))
    .expect("credential tuple is serializable");
    format!("{:x}", Sha256::digest(bytes))
}

fn circuit_epoch(key: &StoredProviderCatalogKey, api_format: &str) -> Option<u64> {
    match key
        .circuit_breaker_by_format
        .as_ref()
        .and_then(|v| v.get(api_format))
        .and_then(|v| v.get("circuit_epoch"))
    {
        None => Some(0),
        Some(value) => value.as_u64(),
    }
}

fn source_fence(key: &StoredProviderCatalogKey, api_format: &str, kind: ProbeLeaseKind) -> Value {
    let health = key
        .health_by_format
        .as_ref()
        .and_then(|v| v.get(api_format));
    let circuit = key
        .circuit_breaker_by_format
        .as_ref()
        .and_then(|v| v.get(api_format));
    let get = |payload: Option<&Value>, field: &str| {
        payload
            .and_then(|v| v.get(field))
            .cloned()
            .unwrap_or(Value::Null)
    };
    // Preserve both recovery sources: a new Retry-After must also invalidate a
    // circuit lease, and a new circuit must invalidate a rate-limit lease.
    json!({
        "kind": kind,
        "open": get(circuit, "open"),
        "next_probe_at_unix_secs": get(circuit, "next_probe_at_unix_secs"),
        "rate_limit_cooldown_until_unix_secs": get(health, "rate_limit_cooldown_until_unix_secs"),
        "consecutive_rate_limits": get(health, "consecutive_rate_limits"),
    })
}

fn health_update(key: &StoredProviderCatalogKey) -> ProviderCatalogKeyHealthStateUpdate {
    ProviderCatalogKeyHealthStateUpdate {
        key_id: key.id.clone(),
        expected_encrypted_auth_config: key.encrypted_auth_config.clone(),
        expected_health_by_format: key.health_by_format.clone(),
        expected_circuit_breaker_by_format: key.circuit_breaker_by_format.clone(),
        health_by_format: key.health_by_format.clone(),
        circuit_breaker_by_format: key.circuit_breaker_by_format.clone(),
    }
}

fn edited_payload<'a>(
    update: &'a mut ProviderCatalogKeyHealthStateUpdate,
    report: &ProbeLeaseReport,
) -> Option<&'a mut serde_json::Map<String, Value>> {
    match report.kind {
        ProbeLeaseKind::Circuit => update.circuit_breaker_by_format.as_mut(),
        ProbeLeaseKind::RateLimit => update.health_by_format.as_mut(),
    }?
    .get_mut(&report.api_format)?
    .as_object_mut()
}

fn set_deadline(
    payload: &mut serde_json::Map<String, Value>,
    kind: ProbeLeaseKind,
    until: Option<u64>,
) {
    payload.insert(
        kind.deadline_field().into(),
        until.map_or(Value::Null, |until| json!(until)),
    );
    if kind == ProbeLeaseKind::Circuit {
        let formatted = until
            .and_then(|until| i64::try_from(until).ok())
            .and_then(|until| chrono::DateTime::from_timestamp(until, 0))
            .map(|at| at.to_rfc3339());
        payload.insert("half_open_until".into(), json!(formatted));
    }
}

fn project_claim(
    key: &StoredProviderCatalogKey,
    report: &ProbeLeaseReport,
    now: u64,
) -> Option<ProviderCatalogKeyHealthStateUpdate> {
    let mut update = health_update(key);
    match report.kind {
        ProbeLeaseKind::Circuit => {
            update.circuit_breaker_by_format = Some(project_local_key_circuit_probe_reservation(
                key.circuit_breaker_by_format.as_ref(),
                &report.api_format,
                now,
            )?)
        }
        ProbeLeaseKind::RateLimit => {
            update.health_by_format = Some(project_local_rate_limit_probe_reservation(
                key.health_by_format.as_ref(),
                &report.api_format,
                now,
            )?)
        }
    }
    let payload = edited_payload(&mut update, report)?;
    payload.insert(
        report.kind.owner_field().into(),
        serde_json::to_value(report).expect("lease report is serializable"),
    );
    set_deadline(
        payload,
        report.kind,
        Some(now.saturating_add(LEASE_TTL_SECS)),
    );
    Some(update)
}

fn project_owned_update(
    key: &StoredProviderCatalogKey,
    report: &ProbeLeaseReport,
    now: u64,
    release: bool,
) -> Option<ProviderCatalogKeyHealthStateUpdate> {
    if !(if release {
        report.same_owner_generation(key)
    } else {
        report.matches(key)
    }) || (!release && !report.expires_at(key).is_some_and(|until| now < until))
    {
        return None;
    }
    let mut update = health_update(key);
    let payload = edited_payload(&mut update, report)?;
    if release {
        payload.remove(report.kind.owner_field());
    }
    // A terminal effect may already have established a new cooldown. Remove
    // our obsolete owner metadata, but leave every new deadline untouched.
    if !release || report.source == source_fence(key, &report.api_format, report.kind) {
        set_deadline(
            payload,
            report.kind,
            if release {
                None
            } else {
                Some(now.saturating_add(LEASE_TTL_SECS))
            },
        );
    }
    Some(update)
}

fn settlement(
    identity: &LeaseIdentity,
    operation_id: &str,
    started_at: u64,
    update: ProviderCatalogKeyHealthStateUpdate,
) -> ProviderCatalogKeyHealthSettlement {
    ProviderCatalogKeyHealthSettlement {
        attempt_id: operation_id.into(),
        api_format: identity.report.api_format.clone(),
        policy_version: super::chat_health::CHAT_HEALTH_POLICY_VERSION as u32,
        attempt_started_at_unix_secs: started_at,
        expected_credential: identity.credential.clone(),
        expected_encrypted_auth_config: identity.encrypted_auth_config.clone(),
        expected_circuit_epoch: identity.report.circuit_epoch,
        expected_probe_lease_expires_at_unix_secs: None,
        update,
    }
}

pub(crate) async fn claim_managed_probe(
    state: &AppState,
    key_id: &str,
    api_format: &str,
    kind: ProbeLeaseKind,
) -> Result<LocalProbeLeaseClaim, GatewayError> {
    tokio::time::timeout(
        OPERATION_TIMEOUT,
        claim_managed_probe_inner(state, key_id.trim(), api_format.trim(), kind),
    )
    .await
    .map_err(|_| GatewayError::Internal("probe lease claim timed out".into()))?
}

async fn claim_managed_probe_inner(
    state: &AppState,
    key_id: &str,
    api_format: &str,
    kind: ProbeLeaseKind,
) -> Result<LocalProbeLeaseClaim, GatewayError> {
    if key_id.is_empty() || api_format.is_empty() {
        return Err(GatewayError::Internal(
            "probe lease requires a key and format".into(),
        ));
    }
    let owner_token = uuid::Uuid::new_v4().to_string();
    let operation_id = format!("probe-lease-claim:{owner_token}");
    let started_at = current_unix_secs();
    let mut identity: Option<LeaseIdentity> = None;
    for _ in 0..CAS_ATTEMPTS {
        let Some(key) = state
            .read_provider_catalog_keys_by_ids_strong(&[key_id.to_owned()])
            .await?
            .into_iter()
            .next()
        else {
            return Ok(LocalProbeLeaseClaim::Unavailable);
        };
        let now = current_unix_secs();
        let required = match kind {
            ProbeLeaseKind::Circuit => is_provider_key_circuit_open(&key, api_format),
            ProbeLeaseKind::RateLimit => {
                provider_key_rate_limit_cooldown(&key, api_format).is_some()
            }
        };
        if !required {
            return Ok(LocalProbeLeaseClaim::NotRequired);
        }
        if identity.is_none() {
            let Some(provider) = state
                .read_provider_catalog_providers_by_ids(std::slice::from_ref(&key.provider_id))
                .await?
                .into_iter()
                .next()
            else {
                return Ok(LocalProbeLeaseClaim::Unavailable);
            };
            let Some(epoch) = circuit_epoch(&key, api_format) else {
                return Ok(LocalProbeLeaseClaim::Unavailable);
            };
            identity = Some(LeaseIdentity {
                report: ProbeLeaseReport {
                    owner_token: owner_token.clone(),
                    kind,
                    key_id: key.id.clone(),
                    api_format: api_format.into(),
                    credential_fingerprint: credential_fingerprint(&key),
                    circuit_epoch: epoch,
                    source: source_fence(&key, api_format, kind),
                },
                credential: ProviderCatalogKeyOAuthCredentialFence {
                    encrypted_api_key: key.encrypted_api_key.clone(),
                    auth_type: key.auth_type.clone(),
                    provider_id: key.provider_id.clone(),
                    provider_type: provider.provider_type,
                },
                encrypted_auth_config: key.encrypted_auth_config.clone(),
            });
        }
        let identity = identity.as_ref().expect("identity initialized");
        if identity.report.credential_fingerprint != credential_fingerprint(&key)
            || identity.report.source != source_fence(&key, api_format, kind)
            || Some(identity.report.circuit_epoch) != circuit_epoch(&key, api_format)
        {
            return Ok(LocalProbeLeaseClaim::Unavailable);
        }
        let Some(update) = project_claim(&key, &identity.report, now) else {
            return Ok(LocalProbeLeaseClaim::Unavailable);
        };
        // Install cancellation cleanup before awaiting the committing write.
        // The first renewal is later than the complete claim timeout.
        let pending = LocalProbeLeaseGuard::start(
            state.clone(),
            identity.clone(),
            now.saturating_add(LEASE_TTL_SECS),
        );
        match state
            .settle_provider_catalog_key_health_attempt(&settlement(
                identity,
                &operation_id,
                started_at,
                update,
            ))
            .await?
        {
            ProviderCatalogKeyHealthSettlementResult::Applied => {
                return Ok(LocalProbeLeaseClaim::Acquired(pending))
            }
            ProviderCatalogKeyHealthSettlementResult::Conflict => {
                pending.finish().await;
                tokio::task::yield_now().await;
            }
            _ => return Ok(LocalProbeLeaseClaim::Unavailable),
        }
    }
    Ok(LocalProbeLeaseClaim::Unavailable)
}

async fn mutate_owned(
    state: &AppState,
    identity: &LeaseIdentity,
    release: bool,
) -> Result<Option<u64>, ProbeLeaseLoss> {
    let operation_id = format!(
        "probe-lease:{}:{}",
        identity.report.owner_token,
        uuid::Uuid::new_v4()
    );
    let started_at = current_unix_secs();
    for _ in 0..CAS_ATTEMPTS {
        let key = state
            .read_provider_catalog_keys_by_ids_strong(std::slice::from_ref(&identity.report.key_id))
            .await
            .map_err(|_| ProbeLeaseLoss::StorageUnavailable)?
            .into_iter()
            .next();
        let Some(key) = key else {
            return if release {
                Ok(None)
            } else {
                Err(ProbeLeaseLoss::OwnershipChanged)
            };
        };
        let now = current_unix_secs();
        if !release
            && identity
                .report
                .expires_at(&key)
                .is_some_and(|until| now >= until)
        {
            return Err(ProbeLeaseLoss::Expired);
        }
        // Do not start a write that could complete after the old lease expires.
        // The worker bounds the complete mutation to OPERATION_TIMEOUT.
        if !release
            && identity
                .report
                .expires_at(&key)
                .is_some_and(|until| until <= now.saturating_add(OPERATION_TIMEOUT.as_secs() + 1))
        {
            return Err(ProbeLeaseLoss::RenewalDeadline);
        }
        let Some(update) = project_owned_update(&key, &identity.report, now, release) else {
            return if release {
                Ok(None)
            } else {
                Err(ProbeLeaseLoss::OwnershipChanged)
            };
        };
        match state
            .settle_provider_catalog_key_health_attempt(&settlement(
                identity,
                &operation_id,
                started_at,
                update,
            ))
            .await
            .map_err(|_| ProbeLeaseLoss::StorageUnavailable)?
        {
            ProviderCatalogKeyHealthSettlementResult::Applied => {
                return Ok(if release {
                    None
                } else {
                    Some(now.saturating_add(LEASE_TTL_SECS))
                })
            }
            ProviderCatalogKeyHealthSettlementResult::Conflict => tokio::task::yield_now().await,
            _ => {
                return if release {
                    Ok(None)
                } else {
                    Err(ProbeLeaseLoss::OwnershipChanged)
                }
            }
        }
    }
    Err(ProbeLeaseLoss::Contention)
}

async fn run_lease_worker(
    state: AppState,
    identity: LeaseIdentity,
    mut stop: oneshot::Receiver<()>,
    status: watch::Sender<ProbeLeaseStatus>,
) {
    let mut loss = None;
    loop {
        tokio::select! {
            biased;
            _ = &mut stop => break,
            _ = tokio::time::sleep(RENEW_INTERVAL) => {}
        }
        let renewed = tokio::select! {
            biased;
            _ = &mut stop => break,
            renewed = tokio::time::timeout(OPERATION_TIMEOUT, mutate_owned(&state, &identity, false)) => renewed,
        };
        match renewed {
            Ok(Ok(Some(until_unix_secs))) => {
                status.send_replace(ProbeLeaseStatus::Active { until_unix_secs });
            }
            other => {
                let reason = match other {
                    Ok(Err(reason)) => reason,
                    _ => ProbeLeaseLoss::StorageUnavailable,
                };
                status.send_replace(ProbeLeaseStatus::Lost(reason));
                loss = Some(reason);
                break;
            }
        }
    }
    let released =
        tokio::time::timeout(OPERATION_TIMEOUT, mutate_owned(&state, &identity, true)).await;
    if !matches!(released, Ok(Ok(_))) {
        warn!(event_name = "probe_lease_release_failed", key_id = %identity.report.key_id, api_format = %identity.report.api_format, "probe lease will expire by TTL");
        if loss.is_none() {
            loss = Some(ProbeLeaseLoss::StorageUnavailable);
        }
    }
    status.send_replace(loss.map_or(ProbeLeaseStatus::Released, ProbeLeaseStatus::Lost));
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider;

    use super::*;
    use crate::data::GatewayDataState;

    const FORMAT: &str = "openai:chat";

    fn key(kind: ProbeLeaseKind) -> StoredProviderCatalogKey {
        StoredProviderCatalogKey::new("key-1".into(), "provider-1".into(), "probe".into(), "api_key".into(), None, true)
            .unwrap()
            .with_health_fields(
                Some(json!({FORMAT: {"health_score": 0.4, "consecutive_failures": 3, "rate_limit_cooldown_until_unix_secs": 90, "consecutive_rate_limits": 1}})),
                Some(json!({FORMAT: {"open": kind == ProbeLeaseKind::Circuit, "next_probe_at_unix_secs": 90, "circuit_epoch": 3}})),
            )
    }

    fn identity(
        key: &StoredProviderCatalogKey,
        kind: ProbeLeaseKind,
        token: &str,
    ) -> LeaseIdentity {
        LeaseIdentity {
            report: ProbeLeaseReport {
                owner_token: token.into(),
                kind,
                key_id: key.id.clone(),
                api_format: FORMAT.into(),
                credential_fingerprint: credential_fingerprint(key),
                circuit_epoch: 3,
                source: source_fence(key, FORMAT, kind),
            },
            credential: ProviderCatalogKeyOAuthCredentialFence {
                encrypted_api_key: key.encrypted_api_key.clone(),
                auth_type: key.auth_type.clone(),
                provider_id: key.provider_id.clone(),
                provider_type: "custom".into(),
            },
            encrypted_auth_config: key.encrypted_auth_config.clone(),
        }
    }

    fn apply(key: &mut StoredProviderCatalogKey, update: ProviderCatalogKeyHealthStateUpdate) {
        key.health_by_format = update.health_by_format;
        key.circuit_breaker_by_format = update.circuit_breaker_by_format;
    }

    #[test]
    fn virtual_time_renewal_extends_exclusion_beyond_sixty_seconds() {
        for kind in [ProbeLeaseKind::Circuit, ProbeLeaseKind::RateLimit] {
            let mut key = key(kind);
            let owner = identity(&key, kind, "owner-a");
            let update = project_claim(&key, &owner.report, 100).unwrap();
            apply(&mut key, update);
            for now in [120, 140, 160, 180] {
                let update = project_owned_update(&key, &owner.report, now, false).unwrap();
                apply(&mut key, update);
            }
            let competitor = identity(&key, kind, "owner-b");
            assert!(project_claim(&key, &competitor.report, 181).is_none());
            assert_eq!(owner.report.expires_at(&key), Some(240));
            assert!(probe_lease_report_is_current(
                &key,
                &serde_json::to_value(&owner.report).unwrap(),
                181
            ));
            assert_eq!(
                key.health_by_format.as_ref().unwrap()[FORMAT]["health_score"],
                0.4
            );
            assert_eq!(
                key.health_by_format.as_ref().unwrap()[FORMAT]["consecutive_failures"],
                3
            );
            assert!(project_owned_update(&key, &owner.report, 240, false).is_none());
            assert!(!probe_lease_report_is_current(
                &key,
                &serde_json::to_value(&owner.report).unwrap(),
                240
            ));
        }
    }

    #[test]
    fn old_owner_cannot_release_new_owner_epoch_credential_or_cooldown() {
        for kind in [ProbeLeaseKind::Circuit, ProbeLeaseKind::RateLimit] {
            let mut original = key(kind);
            let old = identity(&original, kind, "old-owner");
            let update = project_claim(&original, &old.report, 100).unwrap();
            apply(&mut original, update);

            let mut replacement = original.clone();
            let new = identity(&replacement, kind, "new-owner");
            let update = project_claim(&replacement, &new.report, 161).unwrap();
            apply(&mut replacement, update);
            assert!(project_owned_update(&replacement, &old.report, 162, true).is_none());
            assert!(project_owned_update(&replacement, &old.report, 162, false).is_none());
            assert!(new.report.matches(&replacement));

            let mut new_epoch = original.clone();
            new_epoch.circuit_breaker_by_format.as_mut().unwrap()[FORMAT]["circuit_epoch"] =
                json!(4);
            assert!(project_owned_update(&new_epoch, &old.report, 120, true).is_none());
            assert!(project_owned_update(&new_epoch, &old.report, 120, false).is_none());

            let mut new_cooldown = original.clone();
            new_cooldown.health_by_format.as_mut().unwrap()[FORMAT]
                ["rate_limit_cooldown_until_unix_secs"] = json!(600);
            let released = project_owned_update(&new_cooldown, &old.report, 120, true).unwrap();
            apply(&mut new_cooldown, released);
            assert!(project_owned_update(&new_cooldown, &old.report, 120, false).is_none());
            assert_eq!(
                new_cooldown.health_by_format.as_ref().unwrap()[FORMAT]
                    ["rate_limit_cooldown_until_unix_secs"],
                600
            );

            let mut new_credential = original.clone();
            new_credential.encrypted_api_key = Some("replacement".into());
            assert!(project_owned_update(&new_credential, &old.report, 120, true).is_none());
            assert!(project_owned_update(&new_credential, &old.report, 120, false).is_none());

            let release = project_owned_update(&original, &old.report, 120, true).unwrap();
            apply(&mut original, release);
            assert!(project_owned_update(&original, &old.report, 121, false).is_none());
            assert_eq!(
                original.health_by_format.as_ref().unwrap()[FORMAT]["health_score"],
                0.4
            );
            assert_eq!(
                original.health_by_format.as_ref().unwrap()[FORMAT]
                    ["rate_limit_cooldown_until_unix_secs"],
                90
            );
            assert_eq!(
                original.circuit_breaker_by_format.as_ref().unwrap()[FORMAT]
                    ["next_probe_at_unix_secs"],
                90
            );
            assert!(
                project_claim(&original, &identity(&original, kind, "next").report, 121).is_some()
            );
        }
    }

    fn state(repository: Arc<InMemoryProviderCatalogReadRepository>) -> AppState {
        AppState::new().unwrap().with_data_state_for_tests(
            GatewayDataState::with_provider_catalog_repository_for_tests(repository),
        )
    }

    #[tokio::test]
    async fn memory_cas_admits_one_guard_and_drop_releases_only_its_slot() {
        for kind in [ProbeLeaseKind::Circuit, ProbeLeaseKind::RateLimit] {
            let provider = StoredProviderCatalogProvider::new(
                "provider-1".into(),
                "Provider".into(),
                None,
                "custom".into(),
            )
            .unwrap();
            let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
                vec![provider],
                vec![],
                vec![key(kind)],
            ));
            let first_state = state(repository.clone());
            let second_state = state(repository);
            let (first, second) = tokio::join!(
                claim_managed_probe(&first_state, "key-1", FORMAT, kind),
                claim_managed_probe(&second_state, "key-1", FORMAT, kind),
            );
            let mut guard = match (first.unwrap(), second.unwrap()) {
                (LocalProbeLeaseClaim::Acquired(guard), LocalProbeLeaseClaim::Unavailable)
                | (LocalProbeLeaseClaim::Unavailable, LocalProbeLeaseClaim::Acquired(guard)) => {
                    guard
                }
                other => panic!("expected exactly one owner, got {other:?}"),
            };
            let captured = guard.report_context();
            let stored = first_state
                .read_provider_catalog_keys_by_ids_strong(&["key-1".into()])
                .await
                .unwrap()
                .remove(0);
            assert!(probe_lease_report_is_current(
                &stored,
                &captured,
                current_unix_secs()
            ));
            let worker = guard.worker.take().unwrap();
            drop(guard);
            tokio::time::timeout(Duration::from_secs(2), worker)
                .await
                .unwrap()
                .unwrap();
            let stored = second_state
                .read_provider_catalog_keys_by_ids_strong(&["key-1".into()])
                .await
                .unwrap()
                .remove(0);
            assert!(!probe_lease_report_is_current(
                &stored,
                &captured,
                current_unix_secs()
            ));
            assert_eq!(
                stored.health_by_format.as_ref().unwrap()[FORMAT]["health_score"],
                0.4
            );
            let LocalProbeLeaseClaim::Acquired(next) =
                claim_managed_probe(&second_state, "key-1", FORMAT, kind)
                    .await
                    .unwrap()
            else {
                panic!("drop must release admission");
            };
            let stored = second_state
                .read_provider_catalog_keys_by_ids_strong(&["key-1".into()])
                .await
                .unwrap()
                .remove(0);
            let mut terminal = health_update(&stored);
            let new_cooldown = current_unix_secs() + 600;
            terminal.health_by_format.as_mut().unwrap()[FORMAT]
                ["rate_limit_cooldown_until_unix_secs"] = json!(new_cooldown);
            assert!(second_state
                .compare_and_update_provider_catalog_key_health_state(&terminal)
                .await
                .unwrap());
            assert_eq!(next.finish().await, ProbeLeaseStatus::Released);
            let stored = second_state
                .read_provider_catalog_keys_by_ids_strong(&["key-1".into()])
                .await
                .unwrap()
                .remove(0);
            assert_eq!(
                stored.health_by_format.as_ref().unwrap()[FORMAT]
                    ["rate_limit_cooldown_until_unix_secs"],
                new_cooldown
            );
        }
    }
}
