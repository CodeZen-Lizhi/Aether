use super::AdminAppState;
use crate::GatewayError;

impl<'a> AdminAppState<'a> {
    pub(crate) async fn get_ldap_module_config(
        &self,
    ) -> Result<Option<aether_data::repository::auth_modules::StoredLdapModuleConfig>, GatewayError>
    {
        self.app.get_ldap_module_config().await
    }

    pub(crate) async fn list_oauth_provider_configs(
        &self,
    ) -> Result<
        Vec<aether_data::repository::oauth_providers::StoredOAuthProviderConfig>,
        GatewayError,
    > {
        self.app.list_oauth_provider_configs().await
    }

    pub(crate) async fn get_oauth_provider_config(
        &self,
        provider_type: &str,
    ) -> Result<
        Option<aether_data::repository::oauth_providers::StoredOAuthProviderConfig>,
        GatewayError,
    > {
        self.app.get_oauth_provider_config(provider_type).await
    }

    pub(crate) async fn upsert_oauth_provider_config(
        &self,
        record: &aether_data::repository::oauth_providers::UpsertOAuthProviderConfigRecord,
    ) -> Result<
        Option<aether_data::repository::oauth_providers::StoredOAuthProviderConfig>,
        GatewayError,
    > {
        self.app.upsert_oauth_provider_config(record).await
    }

    pub(crate) async fn delete_oauth_provider_config(
        &self,
        provider_type: &str,
    ) -> Result<bool, GatewayError> {
        self.app.delete_oauth_provider_config(provider_type).await
    }

    pub(crate) async fn count_locked_users_if_oauth_provider_disabled(
        &self,
        provider_type: &str,
        ldap_exclusive: bool,
    ) -> Result<usize, GatewayError> {
        self.app
            .count_locked_users_if_oauth_provider_disabled(provider_type, ldap_exclusive)
            .await
    }

    pub(crate) async fn get_management_token_with_user(
        &self,
        token_id: &str,
    ) -> Result<
        Option<aether_data::repository::management_tokens::StoredManagementTokenWithUser>,
        GatewayError,
    > {
        self.app.get_management_token_with_user(token_id).await
    }

    pub(crate) async fn delete_management_token(
        &self,
        token_id: &str,
    ) -> Result<bool, GatewayError> {
        self.app.delete_management_token(token_id).await
    }

    pub(crate) async fn create_management_token(
        &self,
        record: &aether_data::repository::management_tokens::CreateManagementTokenRecord,
    ) -> Result<
        crate::LocalMutationOutcome<
            aether_data::repository::management_tokens::StoredManagementToken,
        >,
        GatewayError,
    > {
        self.app.create_management_token(record).await
    }

    pub(crate) async fn update_management_token(
        &self,
        record: &aether_data::repository::management_tokens::UpdateManagementTokenRecord,
    ) -> Result<
        crate::LocalMutationOutcome<
            aether_data::repository::management_tokens::StoredManagementToken,
        >,
        GatewayError,
    > {
        self.app.update_management_token(record).await
    }

    pub(crate) async fn list_management_tokens(
        &self,
        query: &aether_data::repository::management_tokens::ManagementTokenListQuery,
    ) -> Result<
        aether_data::repository::management_tokens::StoredManagementTokenListPage,
        GatewayError,
    > {
        self.app.list_management_tokens(query).await
    }

    pub(crate) async fn set_management_token_active(
        &self,
        token_id: &str,
        is_active: bool,
    ) -> Result<
        Option<aether_data::repository::management_tokens::StoredManagementToken>,
        GatewayError,
    > {
        self.app
            .set_management_token_active(token_id, is_active)
            .await
    }

    pub(crate) async fn regenerate_management_token_secret(
        &self,
        mutation: &aether_data::repository::management_tokens::RegenerateManagementTokenSecret,
    ) -> Result<
        crate::LocalMutationOutcome<
            aether_data::repository::management_tokens::StoredManagementToken,
        >,
        GatewayError,
    > {
        self.app.regenerate_management_token_secret(mutation).await
    }

    pub(crate) async fn upsert_ldap_module_config(
        &self,
        config: &aether_data::repository::auth_modules::StoredLdapModuleConfig,
    ) -> Result<Option<aether_data::repository::auth_modules::StoredLdapModuleConfig>, GatewayError>
    {
        self.app.upsert_ldap_module_config(config).await
    }

    pub(crate) async fn count_active_local_admin_users_with_valid_password(
        &self,
    ) -> Result<u64, GatewayError> {
        self.app
            .count_active_local_admin_users_with_valid_password()
            .await
    }

    pub(crate) async fn list_enabled_oauth_module_providers(
        &self,
    ) -> Result<
        Vec<aether_data::repository::auth_modules::StoredOAuthProviderModuleConfig>,
        GatewayError,
    > {
        self.app.list_enabled_oauth_module_providers().await
    }
}
