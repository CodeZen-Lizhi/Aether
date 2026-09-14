//! Recovery probe ownership for one real WebSocket provider attempt.

use aether_contracts::ExecutionPlan;
use serde_json::{json, Value};

use crate::orchestration::{
    try_claim_managed_local_circuit_probe, try_claim_managed_local_rate_limit_probe,
    LocalProbeLeaseClaim, LocalProbeLeaseGuard, ProbeLeaseLoss, ProbeLeaseStatus,
    PROBE_LEASES_REPORT_FIELD,
};
use crate::{AppState, GatewayError};

pub(super) const PROBE_LEASE_LOST: &str = "responses_websocket_probe_lease_lost";

#[derive(Default)]
pub(super) struct ResponsesProbeLeases {
    circuit: Option<LocalProbeLeaseGuard>,
    rate_limit: Option<LocalProbeLeaseGuard>,
}

impl ResponsesProbeLeases {
    /// Called only after all execution admission gates. Planner enumeration
    /// must not reserve an ownerless chat probe before this claim.
    pub(super) async fn claim(
        state: &AppState,
        plan: &ExecutionPlan,
        report_context: &mut Option<Value>,
    ) -> Result<Self, GatewayError> {
        let circuit = required_guard(
            try_claim_managed_local_circuit_probe(state, &plan.key_id, &plan.provider_api_format)
                .await?,
        )?;
        let rate_limit = required_guard(
            try_claim_managed_local_rate_limit_probe(
                state,
                &plan.key_id,
                &plan.provider_api_format,
            )
            .await?,
        )?;
        let guards = Self {
            circuit,
            rate_limit,
        };
        if guards.has_lost() {
            return Err(probe_unavailable());
        }
        let report = report_context.get_or_insert_with(|| json!({}));
        let object = report.as_object_mut().ok_or_else(|| {
            GatewayError::Internal("WebSocket attempt report context must be an object".into())
        })?;
        object.insert(
            PROBE_LEASES_REPORT_FIELD.to_string(),
            Value::Array(
                [&guards.circuit, &guards.rate_limit]
                    .into_iter()
                    .flatten()
                    .map(LocalProbeLeaseGuard::report_context)
                    .collect(),
            ),
        );
        Ok(guards)
    }

    fn has_lost(&self) -> bool {
        [&self.circuit, &self.rate_limit]
            .into_iter()
            .flatten()
            .any(|guard| !matches!(guard.status(), ProbeLeaseStatus::Active { .. }))
    }

    pub(super) async fn lost(&mut self) -> ProbeLeaseLoss {
        tokio::select! {
            loss = optional_loss(self.circuit.as_mut()) => loss,
            loss = optional_loss(self.rate_limit.as_mut()) => loss,
        }
    }

    pub(super) async fn run<T>(
        &mut self,
        work: impl std::future::Future<Output = Result<T, &'static str>>,
    ) -> Result<T, &'static str> {
        if self.has_lost() {
            return Err(PROBE_LEASE_LOST);
        }
        tokio::select! {
            biased;
            _ = self.lost() => Err(PROBE_LEASE_LOST),
            result = work => result,
        }
    }

    /// Business effects may supersede the lease's epoch/cooldown. Finishing
    /// releases ownership only; it never rewrites the known provider outcome.
    pub(super) async fn finish(self) {
        if let Some(guard) = self.circuit {
            let _ = guard.finish().await;
        }
        if let Some(guard) = self.rate_limit {
            let _ = guard.finish().await;
        }
    }
}

async fn optional_loss(guard: Option<&mut LocalProbeLeaseGuard>) -> ProbeLeaseLoss {
    match guard {
        Some(guard) => guard.lost().await,
        None => std::future::pending().await,
    }
}

fn required_guard(
    claim: LocalProbeLeaseClaim,
) -> Result<Option<LocalProbeLeaseGuard>, GatewayError> {
    match claim {
        LocalProbeLeaseClaim::NotRequired => Ok(None),
        LocalProbeLeaseClaim::Acquired(guard) => Ok(Some(guard)),
        LocalProbeLeaseClaim::Unavailable => Err(probe_unavailable()),
    }
}

fn probe_unavailable() -> GatewayError {
    GatewayError::Client {
        status: http::StatusCode::SERVICE_UNAVAILABLE,
        message: "The selected provider cannot admit this recovery probe".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::GatewayDataState;
    use crate::orchestration::probe_lease_report_is_current;
    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::provider_catalog::{
        StoredProviderCatalogKey, StoredProviderCatalogProvider,
    };
    use std::sync::Arc;

    fn state() -> AppState {
        let provider = StoredProviderCatalogProvider::new(
            "provider-1".into(),
            "Provider".into(),
            None,
            "custom".into(),
        )
        .unwrap();
        let key = StoredProviderCatalogKey::new("key-1".into(), "provider-1".into(), "probe".into(), "api_key".into(), None, true).unwrap()
            .with_health_fields(
                Some(json!({"openai:responses": {"health_score": 0.0, "consecutive_failures": 4, "rate_limit_cooldown_until_unix_secs": 90, "consecutive_rate_limits": 1}})),
                Some(json!({"openai:responses": {"open": true, "next_probe_at_unix_secs": 90, "circuit_epoch": 3}})),
            );
        AppState::new().unwrap().with_data_state_for_tests(
            GatewayDataState::with_provider_catalog_repository_for_tests(Arc::new(
                InMemoryProviderCatalogReadRepository::seed(vec![provider], vec![], vec![key]),
            )),
        )
    }

    fn plan() -> ExecutionPlan {
        serde_json::from_value(json!({
            "request_id": "probe-request", "provider_id": "provider-1", "endpoint_id": "endpoint-1", "key_id": "key-1",
            "method": "POST", "url": "http://localhost/v1/responses", "headers": {}, "body": {"json_body": {"model": "test"}},
            "stream": true, "client_api_format": "openai:responses", "provider_api_format": "openai:responses"
        })).unwrap()
    }

    #[tokio::test]
    async fn ws_keeps_both_probe_owners_until_terminal_finish() {
        let state = state();
        let mut report = Some(json!({"chat_health_attempt": {"attempt_id": "ws-attempt"}}));
        let mut guards = ResponsesProbeLeases::claim(&state, &plan(), &mut report)
            .await
            .unwrap();
        let reports = report.as_ref().unwrap()[PROBE_LEASES_REPORT_FIELD]
            .as_array()
            .unwrap()
            .clone();
        assert_eq!(reports.len(), 2);
        assert_eq!(
            report.as_ref().unwrap()["chat_health_attempt"]["attempt_id"],
            "ws-attempt"
        );
        let completed = guards.run(async { Ok("provider-terminal") }).await.unwrap();
        assert_eq!(completed, "provider-terminal");

        let stored = state
            .read_provider_catalog_keys_by_ids_strong(&["key-1".into()])
            .await
            .unwrap()
            .remove(0);
        assert!(reports.iter().all(|report| probe_lease_report_is_current(
            &stored,
            report,
            crate::clock::current_unix_secs()
        )));
        assert!(ResponsesProbeLeases::claim(&state, &plan(), &mut None)
            .await
            .is_err());

        guards.finish().await;
        let stored = state
            .read_provider_catalog_keys_by_ids_strong(&["key-1".into()])
            .await
            .unwrap()
            .remove(0);
        assert!(reports.iter().all(|report| !probe_lease_report_is_current(
            &stored,
            report,
            crate::clock::current_unix_secs()
        )));
        assert_eq!(
            stored.health_by_format.unwrap()["openai:responses"]["health_score"],
            0.0
        );
    }

    #[tokio::test]
    async fn ordinary_ws_attempt_without_probes_does_not_wait_for_lease_events() {
        let mut guards = ResponsesProbeLeases::default();
        assert_eq!(guards.run(async { Ok(7) }).await, Ok(7));
        guards.finish().await;
    }
}
