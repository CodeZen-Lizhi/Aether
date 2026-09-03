use aether_data_contracts::repository::provider_catalog::ProviderCatalogKeyHealthStateUpdate;
use aether_scheduler_core::{
    provider_key_rate_limit_cooldown, provider_key_rate_limit_probe_active_at,
};

use super::project_local_rate_limit_probe_reservation;
use crate::clock::current_unix_secs;
use crate::{AppState, GatewayError};

const PROVIDER_KEY_RATE_LIMIT_PROBE_CAS_MAX_ATTEMPTS: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LocalRateLimitProbeClaim {
    /// This key has no prior 429 cooldown for the provider format.
    NotRequired,
    /// This caller owns the single post-cooldown recovery probe.
    Acquired,
    /// A fresh cooldown or another caller's reservation excludes this key.
    Unavailable,
}

/// Claim the bounded recovery probe allowed after a rate-limit cooldown expires.
///
/// Selection uses a batch strong read, while the health-state compare-and-set
/// below remains the final ownership boundary immediately before a candidate
/// enters execution.
pub(crate) async fn try_claim_local_rate_limit_probe(
    state: &AppState,
    key_id: &str,
    api_format: &str,
) -> Result<LocalRateLimitProbeClaim, GatewayError> {
    let key_id = key_id.trim();
    let api_format = api_format.trim();
    if key_id.is_empty() || api_format.is_empty() {
        return Ok(LocalRateLimitProbeClaim::NotRequired);
    }

    let observed_at_unix_secs = current_unix_secs();
    for _ in 0..PROVIDER_KEY_RATE_LIMIT_PROBE_CAS_MAX_ATTEMPTS {
        let Some(current_key) = state
            .read_provider_catalog_keys_by_ids_strong(&[key_id.to_string()])
            .await?
            .into_iter()
            .next()
        else {
            return Ok(LocalRateLimitProbeClaim::NotRequired);
        };

        let Some(cooldown) = provider_key_rate_limit_cooldown(&current_key, api_format) else {
            return Ok(LocalRateLimitProbeClaim::NotRequired);
        };
        if observed_at_unix_secs < cooldown.until_unix_secs
            || provider_key_rate_limit_probe_active_at(
                &current_key,
                api_format,
                observed_at_unix_secs,
            )
        {
            return Ok(LocalRateLimitProbeClaim::Unavailable);
        }

        let Some(health_by_format) = project_local_rate_limit_probe_reservation(
            current_key.health_by_format.as_ref(),
            api_format,
            observed_at_unix_secs,
        ) else {
            // A concurrent writer changed the payload between the read and
            // projection. Re-read once through the bounded CAS loop.
            tokio::task::yield_now().await;
            continue;
        };
        let update = ProviderCatalogKeyHealthStateUpdate {
            key_id: current_key.id.clone(),
            expected_encrypted_auth_config: current_key.encrypted_auth_config.clone(),
            expected_health_by_format: current_key.health_by_format.clone(),
            expected_circuit_breaker_by_format: current_key.circuit_breaker_by_format.clone(),
            health_by_format: Some(health_by_format),
            circuit_breaker_by_format: current_key.circuit_breaker_by_format,
        };
        if state
            .compare_and_update_provider_catalog_key_health_state(&update)
            .await?
        {
            return Ok(LocalRateLimitProbeClaim::Acquired);
        }
        tokio::task::yield_now().await;
    }

    // Treat contention beyond the bounded retry budget as unavailable. The
    // candidate source will continue to a different key instead of issuing an
    // uncoordinated probe against the recently rate-limited one.
    Ok(LocalRateLimitProbeClaim::Unavailable)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
    use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
    use aether_scheduler_core::provider_key_rate_limit_probe_active_at;
    use serde_json::json;

    use super::{try_claim_local_rate_limit_probe, LocalRateLimitProbeClaim};
    use crate::data::GatewayDataState;
    use crate::AppState;

    fn state_with_expired_rate_limit_cooldown() -> AppState {
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
            Some(json!({
                "openai:chat": {
                    "rate_limit_cooldown_until_unix_secs": 1,
                    "consecutive_rate_limits": 1
                }
            })),
            None,
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
    async fn concurrent_claims_admit_exactly_one_rate_limit_probe() {
        let state = state_with_expired_rate_limit_cooldown();

        let (first, second) = tokio::join!(
            try_claim_local_rate_limit_probe(&state, "key-1", "openai:chat"),
            try_claim_local_rate_limit_probe(&state, "key-1", "openai:chat"),
        );
        let claims = [
            first.expect("first claim should succeed"),
            second.expect("second claim should succeed"),
        ];
        assert_eq!(
            claims
                .iter()
                .filter(|claim| **claim == LocalRateLimitProbeClaim::Acquired)
                .count(),
            1
        );
        assert_eq!(
            claims
                .iter()
                .filter(|claim| **claim == LocalRateLimitProbeClaim::Unavailable)
                .count(),
            1
        );

        let stored = state
            .read_provider_catalog_keys_by_ids(&["key-1".to_string()])
            .await
            .expect("key should reload")
            .into_iter()
            .next()
            .expect("key should exist");
        assert!(provider_key_rate_limit_probe_active_at(
            &stored,
            "openai:chat",
            crate::clock::current_unix_secs(),
        ));
    }
}
