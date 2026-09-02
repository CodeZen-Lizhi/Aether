use std::time::Duration;

use crate::{AppState, GatewayError};

const USER_RUNTIME_JSON_CACHE_TTL: Duration = Duration::from_secs(30);

impl AppState {
    pub(crate) async fn read_user_model_capability_settings(
        &self,
        user_id: &str,
    ) -> Result<Option<serde_json::Value>, GatewayError> {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Ok(None);
        }
        #[cfg(test)]
        if let Some(store) = self.auth_user_model_capability_store.as_ref() {
            if let Some(settings) = store
                .lock()
                .expect("auth user model capability store should lock")
                .get(user_id)
                .cloned()
            {
                return Ok(Some(settings));
            }
        }

        let cache_key = user_id.to_string();
        self.user_model_capability_settings_cache
            .get_or_load(cache_key, USER_RUNTIME_JSON_CACHE_TTL, || async move {
                Ok(self
                    .data
                    .find_export_user_by_id(user_id)
                    .await
                    .map_err(|err| GatewayError::Internal(err.to_string()))?
                    .and_then(|user| user.model_capability_settings))
            })
            .await
    }

    pub(crate) async fn update_user_model_capability_settings(
        &self,
        user_id: &str,
        settings: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_user_model_capability_store.as_ref() {
            let mut guard = store
                .lock()
                .expect("auth user model capability store should lock");
            match settings {
                Some(value) => {
                    guard.insert(user_id.to_string(), value.clone());
                    return Ok(Some(value));
                }
                None => {
                    guard.remove(user_id);
                    return Ok(None);
                }
            }
        }

        self.data
            .update_user_model_capability_settings(user_id, settings)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn read_user_feature_settings(
        &self,
        user_id: &str,
    ) -> Result<Option<serde_json::Value>, GatewayError> {
        let user_id = user_id.trim();
        if user_id.is_empty() {
            return Ok(None);
        }
        let cache_key = user_id.to_string();
        self.user_feature_settings_cache
            .get_or_load(cache_key, USER_RUNTIME_JSON_CACHE_TTL, || async move {
                self.data
                    .read_user_feature_settings(user_id)
                    .await
                    .map_err(|err| GatewayError::Internal(err.to_string()))
            })
            .await
    }

    pub(crate) async fn update_user_feature_settings(
        &self,
        user_id: &str,
        settings: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>, GatewayError> {
        let updated = self
            .data
            .update_user_feature_settings(user_id, settings)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if updated.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(updated)
    }

    pub(crate) async fn find_active_provider_name(
        &self,
        provider_id: &str,
    ) -> Result<Option<String>, GatewayError> {
        self.data
            .find_active_provider_name(provider_id)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))
    }

    pub(crate) async fn initialize_auth_user_wallet(
        &self,
        user_id: &str,
        initial_gift_usd: f64,
        unlimited: bool,
    ) -> Result<Option<aether_data::repository::wallet::StoredWalletSnapshot>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_wallet_store.as_ref() {
            let gift_balance = if unlimited {
                0.0
            } else {
                initial_gift_usd.max(0.0)
            };
            let now_unix_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let wallet = aether_data::repository::wallet::StoredWalletSnapshot::new(
                uuid::Uuid::new_v4().to_string(),
                Some(user_id.to_string()),
                None,
                0.0,
                gift_balance,
                if unlimited {
                    "unlimited".to_string()
                } else {
                    "finite".to_string()
                },
                "USD".to_string(),
                "active".to_string(),
                0.0,
                0.0,
                0.0,
                gift_balance,
                now_unix_secs,
            )
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
            store
                .lock()
                .expect("auth wallet store should lock")
                .insert(wallet.id.clone(), wallet.clone());
            self.invalidate_auth_context_cache();
            return Ok(Some(wallet));
        }

        let wallet = self
            .data
            .initialize_auth_user_wallet(user_id, initial_gift_usd, unlimited)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if wallet.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(wallet)
    }

    pub(crate) async fn initialize_auth_api_key_wallet(
        &self,
        api_key_id: &str,
        initial_gift_usd: f64,
        unlimited: bool,
    ) -> Result<Option<aether_data::repository::wallet::StoredWalletSnapshot>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_wallet_store.as_ref() {
            let gift_balance = if unlimited {
                0.0
            } else {
                initial_gift_usd.max(0.0)
            };
            let now_unix_secs = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64;
            let wallet = aether_data::repository::wallet::StoredWalletSnapshot::new(
                uuid::Uuid::new_v4().to_string(),
                None,
                Some(api_key_id.to_string()),
                0.0,
                gift_balance,
                if unlimited {
                    "unlimited".to_string()
                } else {
                    "finite".to_string()
                },
                "USD".to_string(),
                "active".to_string(),
                0.0,
                0.0,
                0.0,
                gift_balance,
                now_unix_secs,
            )
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
            store
                .lock()
                .expect("auth wallet store should lock")
                .insert(wallet.id.clone(), wallet.clone());
            self.invalidate_auth_context_cache();
            return Ok(Some(wallet));
        }

        let wallet = self
            .data
            .initialize_auth_api_key_wallet(api_key_id, initial_gift_usd, unlimited)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if wallet.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(wallet)
    }

    pub(crate) async fn update_auth_user_wallet_limit_mode(
        &self,
        user_id: &str,
        limit_mode: &str,
    ) -> Result<Option<aether_data::repository::wallet::StoredWalletSnapshot>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_wallet_store.as_ref() {
            let mut guard = store.lock().expect("auth wallet store should lock");
            let Some((wallet_id, wallet)) = guard
                .iter_mut()
                .find(|(_, wallet)| wallet.user_id.as_deref() == Some(user_id))
            else {
                return Ok(None);
            };
            let _ = wallet_id;
            wallet.limit_mode = limit_mode.to_string();
            wallet.updated_at_unix_secs = chrono::Utc::now().timestamp().max(0) as u64;
            let wallet = wallet.clone();
            drop(guard);
            self.invalidate_auth_context_cache();
            return Ok(Some(wallet));
        }

        let wallet = self
            .data
            .update_auth_user_wallet_limit_mode(user_id, limit_mode)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if wallet.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(wallet)
    }

    pub(crate) async fn update_auth_api_key_wallet_limit_mode(
        &self,
        api_key_id: &str,
        limit_mode: &str,
    ) -> Result<Option<aether_data::repository::wallet::StoredWalletSnapshot>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_wallet_store.as_ref() {
            let mut guard = store.lock().expect("auth wallet store should lock");
            let Some((wallet_id, wallet)) = guard
                .iter_mut()
                .find(|(_, wallet)| wallet.api_key_id.as_deref() == Some(api_key_id))
            else {
                return Ok(None);
            };
            let _ = wallet_id;
            wallet.limit_mode = limit_mode.to_string();
            wallet.updated_at_unix_secs = chrono::Utc::now().timestamp().max(0) as u64;
            let wallet = wallet.clone();
            drop(guard);
            self.invalidate_auth_context_cache();
            return Ok(Some(wallet));
        }

        let wallet = self
            .data
            .update_auth_api_key_wallet_limit_mode(api_key_id, limit_mode)
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if wallet.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(wallet)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn update_auth_user_wallet_snapshot(
        &self,
        user_id: &str,
        balance: f64,
        gift_balance: f64,
        limit_mode: &str,
        currency: &str,
        status: &str,
        total_recharged: f64,
        total_consumed: f64,
        total_refunded: f64,
        total_adjusted: f64,
        updated_at_unix_secs: Option<u64>,
    ) -> Result<Option<aether_data::repository::wallet::StoredWalletSnapshot>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_wallet_store.as_ref() {
            let mut guard = store.lock().expect("auth wallet store should lock");
            let Some((_, wallet)) = guard
                .iter_mut()
                .find(|(_, wallet)| wallet.user_id.as_deref() == Some(user_id))
            else {
                return Ok(None);
            };
            wallet.balance = balance;
            wallet.gift_balance = gift_balance;
            wallet.limit_mode = limit_mode.to_string();
            wallet.currency = currency.to_string();
            wallet.status = status.to_string();
            wallet.total_recharged = total_recharged;
            wallet.total_consumed = total_consumed;
            wallet.total_refunded = total_refunded;
            wallet.total_adjusted = total_adjusted;
            if let Some(updated_at_unix_secs) = updated_at_unix_secs {
                wallet.updated_at_unix_secs = updated_at_unix_secs;
            }
            let wallet = wallet.clone();
            drop(guard);
            self.invalidate_auth_context_cache();
            return Ok(Some(wallet));
        }

        let wallet = self
            .data
            .update_auth_user_wallet_snapshot(
                user_id,
                balance,
                gift_balance,
                limit_mode,
                currency,
                status,
                total_recharged,
                total_consumed,
                total_refunded,
                total_adjusted,
                updated_at_unix_secs,
            )
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if wallet.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(wallet)
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn update_auth_api_key_wallet_snapshot(
        &self,
        api_key_id: &str,
        balance: f64,
        gift_balance: f64,
        limit_mode: &str,
        currency: &str,
        status: &str,
        total_recharged: f64,
        total_consumed: f64,
        total_refunded: f64,
        total_adjusted: f64,
        updated_at_unix_secs: Option<u64>,
    ) -> Result<Option<aether_data::repository::wallet::StoredWalletSnapshot>, GatewayError> {
        #[cfg(test)]
        if let Some(store) = self.auth_wallet_store.as_ref() {
            let mut guard = store.lock().expect("auth wallet store should lock");
            let Some((_, wallet)) = guard
                .iter_mut()
                .find(|(_, wallet)| wallet.api_key_id.as_deref() == Some(api_key_id))
            else {
                return Ok(None);
            };
            wallet.balance = balance;
            wallet.gift_balance = gift_balance;
            wallet.limit_mode = limit_mode.to_string();
            wallet.currency = currency.to_string();
            wallet.status = status.to_string();
            wallet.total_recharged = total_recharged;
            wallet.total_consumed = total_consumed;
            wallet.total_refunded = total_refunded;
            wallet.total_adjusted = total_adjusted;
            if let Some(updated_at_unix_secs) = updated_at_unix_secs {
                wallet.updated_at_unix_secs = updated_at_unix_secs;
            }
            let wallet = wallet.clone();
            drop(guard);
            self.invalidate_auth_context_cache();
            return Ok(Some(wallet));
        }

        let wallet = self
            .data
            .update_auth_api_key_wallet_snapshot(
                api_key_id,
                balance,
                gift_balance,
                limit_mode,
                currency,
                status,
                total_recharged,
                total_consumed,
                total_refunded,
                total_adjusted,
                updated_at_unix_secs,
            )
            .await
            .map_err(|err| GatewayError::Internal(err.to_string()))?;
        if wallet.is_some() {
            self.invalidate_auth_context_cache();
        }
        Ok(wallet)
    }
}
