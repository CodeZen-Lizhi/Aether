use super::*;

use aether_data::repository::global_models::InMemoryGlobalModelReadRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::{DatabaseDriver, SqlDatabaseConfig, SqlPoolConfig};
use aether_data_contracts::repository::global_models::StoredAdminGlobalModel;
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogProvider;

fn config() -> DesktopSessionConfig {
    DesktopSessionConfig::new(&"a".repeat(64), "127.0.0.1:8084".parse().unwrap(), true).unwrap()
}

fn local_admin(id: &str) -> StoredUserAuthRecord {
    StoredUserAuthRecord::new(
        id.to_string(),
        Some(format!("{id}@example.com")),
        true,
        id.to_string(),
        Some(bcrypt::hash("ExistingPassword", 4).unwrap()),
        "admin".to_string(),
        "local".to_string(),
        None,
        None,
        None,
        true,
        false,
        Some(chrono::Utc::now()),
        None,
    )
    .unwrap()
}

#[test]
fn desktop_session_config_requires_exact_loopback_and_host_lifetime() {
    for bind in [
        "0.0.0.0:8084",
        "127.0.0.2:8084",
        "[::1]:8084",
        "127.0.0.1:0",
    ] {
        assert_eq!(
            DesktopSessionConfig::new(&"a".repeat(64), bind.parse().unwrap(), true)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput,
        );
    }
    assert!(
        DesktopSessionConfig::new(&"a".repeat(64), "127.0.0.1:8084".parse().unwrap(), false,)
            .is_err()
    );
}

#[test]
fn desktop_session_config_rejects_missing_or_malformed_capabilities_without_disclosing_them() {
    for secret in [
        String::new(),
        "a".repeat(63),
        "a".repeat(65),
        "g".repeat(64),
    ] {
        let error = DesktopSessionConfig::new(&secret, "127.0.0.1:8084".parse().unwrap(), true)
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        if !secret.is_empty() {
            assert!(!error.to_string().contains(&secret));
        }
    }
    assert!(!format!("{:?}", config()).contains(&"a".repeat(64)));
}

#[tokio::test]
async fn desktop_session_bootstraps_empty_identity_once() {
    let state = AppState::new().unwrap();
    assert!(state.desktop_session.is_none());
    let state = state.with_desktop_session(config()).await.unwrap();
    let first = state
        .find_user_auth_by_identifier(DESKTOP_LOCAL_USERNAME)
        .await
        .unwrap()
        .unwrap();
    assert!(is_usable_desktop_admin(&first));
    assert!(first
        .password_hash
        .as_ref()
        .is_some_and(|value| value.starts_with("$2")));
    assert!(!bcrypt::verify(&"a".repeat(64), first.password_hash.as_deref().unwrap()).unwrap());

    let state = state.with_desktop_session(config()).await.unwrap();
    let reused = state
        .find_user_auth_by_id(&first.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first, reused);
    assert_eq!(state.desktop_session.as_ref().unwrap().user_id, first.id);
    assert_eq!(
        state
            .auth_user_store
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn desktop_session_reuses_existing_identity_and_preserves_any_password_hash() {
    let existing = local_admin("existing-desktop-admin");
    for password_hash in [
        existing.password_hash.clone(),
        Some("legacy-unusable-hash".to_string()),
        None,
    ] {
        let mut user = existing.clone();
        user.password_hash = password_hash;
        let state = AppState::new()
            .unwrap()
            .with_auth_users_for_tests([user.clone()]);
        let state = state.with_desktop_session(config()).await.unwrap();
        assert_eq!(state.desktop_session.as_ref().unwrap().user_id, user.id);
        assert_eq!(
            state.find_user_auth_by_id(&user.id).await.unwrap(),
            Some(user)
        );
        assert!(state
            .find_user_auth_by_identifier(DESKTOP_LOCAL_USERNAME)
            .await
            .unwrap()
            .is_none());
    }
}

#[tokio::test]
async fn desktop_session_rejects_multiple_administrators_without_mutating_them() {
    let first = local_admin("admin-one");
    let second = local_admin("admin-two");
    let state = AppState::new()
        .unwrap()
        .with_auth_users_for_tests([first.clone(), second.clone()]);
    let observer = state.clone();
    let error = state.with_desktop_session(config()).await.unwrap_err();
    assert!(error.to_string().contains("多个管理员"));
    for original in [first, second] {
        assert_eq!(
            observer.find_user_auth_by_id(&original.id).await.unwrap(),
            Some(original)
        );
    }
    assert!(observer.desktop_session.is_none());
}

#[tokio::test]
async fn desktop_session_rejects_disabled_foreign_deleted_or_non_admin_identities() {
    let original = local_admin("existing-account");
    let mut disabled = original.clone();
    disabled.is_active = false;
    let mut deleted = original.clone();
    deleted.is_deleted = true;
    let mut foreign = original.clone();
    foreign.auth_source = "oauth".to_string();
    let mut non_admin = original;
    non_admin.role = "user".to_string();

    for existing in [disabled, deleted, foreign, non_admin] {
        let state = AppState::new()
            .unwrap()
            .with_auth_users_for_tests([existing.clone()]);
        let observer = state.clone();
        assert!(state.with_desktop_session(config()).await.is_err());
        assert_eq!(
            observer.find_user_auth_by_id(&existing.id).await.unwrap(),
            Some(existing)
        );
        assert_eq!(
            observer
                .auth_user_store
                .as_ref()
                .unwrap()
                .lock()
                .unwrap()
                .len(),
            1
        );
    }
}

#[tokio::test]
async fn desktop_session_does_not_bootstrap_a_database_with_only_provider_or_model_data() {
    let provider = StoredProviderCatalogProvider::new(
        "existing-provider".to_string(),
        "Existing provider".to_string(),
        None,
        "custom".to_string(),
    )
    .unwrap();
    let provider_data =
        crate::data::GatewayDataState::with_provider_catalog_reader_for_tests(Arc::new(
            InMemoryProviderCatalogReadRepository::seed(vec![provider], vec![], vec![]),
        ));
    let model = StoredAdminGlobalModel::new(
        "existing-model".to_string(),
        "existing-model".to_string(),
        "Existing model".to_string(),
        false,
        None,
        None,
        None,
        None,
        0,
        0,
        0,
        None,
        None,
    )
    .unwrap();
    let model_data = crate::data::GatewayDataState::disabled().with_global_model_reader(Arc::new(
        InMemoryGlobalModelReadRepository::default().with_admin_global_models([model]),
    ));

    for data in [provider_data, model_data] {
        let state = AppState::new().unwrap().with_data_state_for_tests(data);
        let observer = state.clone();
        let error = state.with_desktop_session(config()).await.unwrap_err();
        assert!(error.to_string().contains("不能自动创建"));
        assert!(observer
            .auth_user_store
            .as_ref()
            .unwrap()
            .lock()
            .unwrap()
            .is_empty());
    }
}

#[tokio::test]
async fn desktop_session_bootstrap_and_reuse_use_the_sqlite_repository() {
    let database = SqlDatabaseConfig::new(
        DatabaseDriver::Sqlite,
        "sqlite::memory:".to_string(),
        SqlPoolConfig {
            max_connections: 1,
            min_connections: 1,
            ..SqlPoolConfig::default()
        },
    )
    .unwrap();
    let mut state = AppState::new()
        .unwrap()
        .with_data_config_and_background_isolation(
            crate::GatewayDataConfig::from_database_config(database),
            false,
        )
        .unwrap();
    state.auth_user_store = None;
    state.auth_wallet_store = None;
    state.prepare_database_for_startup().await.unwrap();
    state.run_database_migrations().await.unwrap();

    let state = state.with_desktop_session(config()).await.unwrap();
    let user_id = state.desktop_session.as_ref().unwrap().user_id.clone();
    let original = state.find_user_auth_by_id(&user_id).await.unwrap().unwrap();
    assert_eq!(
        state.read_admin_system_stats().await.unwrap().total_users,
        1
    );
    let state = state.with_desktop_session(config()).await.unwrap();
    assert_eq!(state.desktop_session.as_ref().unwrap().user_id, user_id);
    assert_eq!(
        state.find_user_auth_by_id(&user_id).await.unwrap(),
        Some(original)
    );
    assert_eq!(
        state.read_admin_system_stats().await.unwrap().total_users,
        1
    );
}
