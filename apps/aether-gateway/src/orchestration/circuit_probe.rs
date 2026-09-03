use aether_data_contracts::repository::provider_catalog::ProviderCatalogKeyHealthStateUpdate;
use aether_scheduler_core::{is_provider_key_circuit_open, is_provider_key_circuit_open_at};

use super::project_local_key_circuit_probe_reservation;
use crate::clock::current_unix_secs;
use crate::{AppState, GatewayError};

const PROVIDER_KEY_CIRCUIT_PROBE_CAS_MAX_ATTEMPTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalCircuitProbeClaim {
    /// This key does not need a half-open circuit probe.
    NotRequired,
    /// This caller owns the only half-open probe for this key and format.
    Acquired,
    /// The circuit is still open or another caller already owns its probe.
    Unavailable,
}

/// Claim the single half-open circuit probe after its backoff deadline.
///
/// Candidate selection observes health in a batch, but the CAS below is the
/// final ownership check immediately before the request reaches the upstream.
pub(crate) async fn try_claim_local_circuit_probe(
    state: &AppState,
    key_id: &str,
    api_format: &str,
) -> Result<LocalCircuitProbeClaim, GatewayError> {
    let key_id = key_id.trim();
    let api_format = api_format.trim();
    if key_id.is_empty() || api_format.is_empty() {
        return Ok(LocalCircuitProbeClaim::NotRequired);
    }

    let observed_at_unix_secs = current_unix_secs();
    for _ in 0..PROVIDER_KEY_CIRCUIT_PROBE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids_strong(&[key_id.to_string()])
            .await?
            .into_iter()
            .next()
        else {
            return Ok(LocalCircuitProbeClaim::NotRequired);
        };

        if !is_provider_key_circuit_open(&current_key, api_format) {
            return Ok(LocalCircuitProbeClaim::NotRequired);
        }
        if is_provider_key_circuit_open_at(&current_key, api_format, observed_at_unix_secs) {
            return Ok(LocalCircuitProbeClaim::Unavailable);
        }

        let Some(circuit_breaker_by_format) = project_local_key_circuit_probe_reservation(
            current_key.circuit_breaker_by_format.as_ref(),
            api_format,
            observed_at_unix_secs,
        ) else {
            // Either another caller claimed the half-open slot or the circuit
            // changed between the strong read and the projection.
            return Ok(LocalCircuitProbeClaim::Unavailable);
        };
        let update = ProviderCatalogKeyHealthStateUpdate {
            key_id: current_key.id.clone(),
            expected_encrypted_auth_config: current_key.encrypted_auth_config.clone(),
            expected_health_by_format: current_key.health_by_format.clone(),
            expected_circuit_breaker_by_format: current_key.circuit_breaker_by_format.clone(),
            health_by_format: current_key.health_by_format,
            circuit_breaker_by_format: Some(circuit_breaker_by_format),
        };
        if state
            .compare_and_update_provider_catalog_key_health_state(&update)
            .await?
        {
            return Ok(LocalCircuitProbeClaim::Acquired);
        }
        tokio::task::yield_now().await;
    }

    // Contention must fail closed for an already-circuited key: execution can
    // move to another candidate while the current probe reservation settles.
    Ok(LocalCircuitProbeClaim::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
    use serde_json::json;

    use super::{try_claim_local_circuit_probe, LocalCircuitProbeClaim};
    use crate::data::GatewayDataState;
    use crate::AppState;

    fn state_with_expired_circuit() -> AppState {
        let key = StoredProviderCatalogKey::new(
            "key-1".to_string(),
            "provider-1".to_string(),
            "probe-test".to_string(),
            "api_key".to_string(),
            None,
            true,
        )
        .expect("key should build")
        .with_health_fields(
            None,
            Some(json!({
                "openai:chat": {
                    "open": true,
                    "next_probe_at_unix_secs": 1
                }
            })),
        );
        let repository = Arc::new(InMemoryProviderCatalogReadRepository::seed(
            vec![],
            vec![],
            vec![key],
        ));
        AppState::new()
            .expect("gateway state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_provider_catalog_repository_for_tests(repository),
            )
    }

    #[tokio::test]
    async fn concurrent_claims_admit_exactly_one_half_open_circuit_probe() {
        let state = state_with_expired_circuit();

        let (first, second) = tokio::join!(
            try_claim_local_circuit_probe(&state, "key-1", "openai:chat"),
            try_claim_local_circuit_probe(&state, "key-1", "openai:chat"),
        );
        let claims = [
            first.expect("first claim should succeed"),
            second.expect("second claim should succeed"),
        ];
        assert_eq!(
            claims
                .iter()
                .filter(|claim| **claim == LocalCircuitProbeClaim::Acquired)
                .count(),
            1
        );
        assert_eq!(
            claims
                .iter()
                .filter(|claim| **claim == LocalCircuitProbeClaim::Unavailable)
                .count(),
            1
        );
    }
}
