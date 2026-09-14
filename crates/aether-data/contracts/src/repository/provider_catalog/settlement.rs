use super::{
    ProviderCatalogKeyHealthStateUpdate, ProviderCatalogKeyOAuthCredentialFence,
    StoredProviderCatalogKey,
};
use crate::DataLayerError;

/// Reports retain their original attempt start time across every retry.
pub const PROVIDER_KEY_HEALTH_SETTLEMENT_RETENTION_SECS: u64 = 86_400;
pub const PROVIDER_KEY_HEALTH_PENDING_BATCH_LIMIT: usize = 1024;

/// Immutable terminal fact. The gateway owns the strict, credential-free JSON DTO.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderCatalogKeyHealthPendingFact {
    pub attempt_id: String,
    pub key_id: String,
    pub api_format: String,
    pub policy_version: u32,
    pub attempt_started_at_unix_secs: u64,
    pub observed_at_unix_secs: u64,
    pub fact: serde_json::Value,
}

impl std::fmt::Debug for ProviderCatalogKeyHealthPendingFact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderCatalogKeyHealthPendingFact")
            .field("attempt_id", &self.attempt_id)
            .field("key_id", &self.key_id)
            .field("api_format", &self.api_format)
            .field("policy_version", &self.policy_version)
            .finish_non_exhaustive()
    }
}

impl ProviderCatalogKeyHealthPendingFact {
    pub fn receipt_id(&self) -> Result<String, DataLayerError> {
        health_receipt_id(
            &self.attempt_id,
            &self.key_id,
            &self.api_format,
            self.policy_version,
        )
    }

    pub fn expires_at_unix_secs(&self) -> u64 {
        self.attempt_started_at_unix_secs
            .saturating_add(PROVIDER_KEY_HEALTH_SETTLEMENT_RETENTION_SECS)
    }

    pub fn validate_enqueue(&self, now: u64) -> Result<(), DataLayerError> {
        self.receipt_id()?;
        if self.attempt_started_at_unix_secs > now.saturating_add(60)
            || self.observed_at_unix_secs > now.saturating_add(60)
            || self.observed_at_unix_secs < self.attempt_started_at_unix_secs
            || self.expires_at_unix_secs() <= now
        {
            return Err(DataLayerError::InvalidInput(
                "invalid or expired provider health pending fact timestamp".into(),
            ));
        }
        Ok(())
    }
}

fn health_receipt_id(
    attempt: &str,
    key: &str,
    format: &str,
    version: u32,
) -> Result<String, DataLayerError> {
    if attempt.trim().is_empty()
        || key.trim().is_empty()
        || format.trim().is_empty()
        || version == 0
    {
        return Err(DataLayerError::InvalidInput(
            "invalid provider health settlement identity".into(),
        ));
    }
    serde_json::to_string(&(attempt, key, format, version)).map_err(|_| {
        DataLayerError::InvalidInput("invalid provider health settlement identity".into())
    })
}

/// Atomic receipt insertion and health CAS. Credentials and circuit epoch are
/// captured before execution; only the health CAS projection is refreshed on conflict.
#[derive(Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProviderCatalogKeyHealthSettlement {
    pub attempt_id: String,
    pub api_format: String,
    pub policy_version: u32,
    pub attempt_started_at_unix_secs: u64,
    pub expected_credential: ProviderCatalogKeyOAuthCredentialFence,
    /// Exact nullable ciphertext; unlike the legacy update, None requires SQL NULL.
    pub expected_encrypted_auth_config: Option<String>,
    pub expected_circuit_epoch: u64,
    /// Model probe deadlines are rechecked under the atomic repository write lock.
    #[serde(default)]
    pub expected_probe_lease_expires_at_unix_secs: Option<u64>,
    pub update: ProviderCatalogKeyHealthStateUpdate,
}

impl std::fmt::Debug for ProviderCatalogKeyHealthSettlement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProviderCatalogKeyHealthSettlement")
            .field("attempt_id", &self.attempt_id)
            .field("key_id", &self.update.key_id)
            .field("api_format", &self.api_format)
            .field("policy_version", &self.policy_version)
            .field("expected_circuit_epoch", &self.expected_circuit_epoch)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderCatalogKeyHealthSettlementResult {
    Applied,
    Duplicate,
    /// No receipt committed. Reload state and reproject with the original fences.
    Conflict,
    /// The original attempt belongs to an obsolete credential, policy or circuit.
    StaleGeneration,
    Expired,
    ProbeLeaseExpired,
    MissingKey,
}

impl ProviderCatalogKeyHealthSettlement {
    pub fn validate(&self, now: u64) -> Result<(), DataLayerError> {
        if self.attempt_id.trim().is_empty()
            || self.api_format.trim().is_empty()
            || self.update.key_id.trim().is_empty()
            || self.policy_version == 0
            || self.attempt_started_at_unix_secs > now.saturating_add(60)
        {
            return Err(DataLayerError::InvalidInput(
                "invalid provider health settlement identity or timestamp".into(),
            ));
        }
        Ok(())
    }

    pub fn receipt_id(&self) -> Result<String, DataLayerError> {
        health_receipt_id(
            &self.attempt_id,
            &self.update.key_id,
            &self.api_format,
            self.policy_version,
        )
    }

    pub fn expires_at_unix_secs(&self) -> u64 {
        self.attempt_started_at_unix_secs
            .saturating_add(PROVIDER_KEY_HEALTH_SETTLEMENT_RETENTION_SECS)
    }

    pub fn generation_matches(&self, key: &StoredProviderCatalogKey, provider_type: &str) -> bool {
        let circuit = key
            .circuit_breaker_by_format
            .as_ref()
            .and_then(|v| v.get(&self.api_format));
        let health = key
            .health_by_format
            .as_ref()
            .and_then(|v| v.get(&self.api_format));
        self.expected_credential.encrypted_api_key == key.encrypted_api_key
            && self.expected_credential.auth_type == key.auth_type
            && self.expected_credential.provider_id == key.provider_id
            && self.expected_credential.provider_type == provider_type
            && self.expected_encrypted_auth_config == key.encrypted_auth_config
            && circuit
                .and_then(|v| v.get("circuit_epoch"))
                .map_or(self.expected_circuit_epoch == 0, |epoch| {
                    epoch.as_u64() == Some(self.expected_circuit_epoch)
                })
            && [health, circuit].into_iter().all(|value| {
                value
                    .and_then(|v| v.get("health_policy_version"))
                    .is_none_or(|version| version.as_u64() == Some(u64::from(self.policy_version)))
            })
    }
}
