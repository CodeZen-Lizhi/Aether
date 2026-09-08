use std::net::SocketAddr;

use aether_gateway::build_router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use http::Request;
use http_body_util::BodyExt;
use tower::ServiceExt;

const GATEWAY_HEADER: &str = "x-aether-gateway";
const ADMIN_USER_ID_HEADER: &str = "x-aether-admin-user-id";
const ADMIN_USER_ROLE_HEADER: &str = "x-aether-admin-user-role";
const ADMIN_SESSION_ID_HEADER: &str = "x-aether-admin-session-id";
const ADMIN_MANAGEMENT_TOKEN_ID_HEADER: &str = "x-aether-admin-management-token-id";

#[tokio::test]
async fn public_admin_routes_reject_unsigned_identity_headers() {
    let router = build_router().expect("gateway should build");

    for identity_header in [ADMIN_SESSION_ID_HEADER, ADMIN_MANAGEMENT_TOKEN_ID_HEADER] {
        for (method, path) in [
            (http::Method::GET, "/api/admin/providers"),
            (
                http::Method::GET,
                "/api/admin/endpoints/providers/provider-1/keys",
            ),
            (http::Method::GET, "/api/admin/endpoints/keys/key-1/reveal"),
        ] {
            let mut request = Request::builder()
                .method(method.clone())
                .uri(path)
                .header(GATEWAY_HEADER, "rust-phase3b")
                .header(ADMIN_USER_ID_HEADER, "1")
                .header(ADMIN_USER_ROLE_HEADER, "admin")
                .header(identity_header, "x")
                .body(Body::empty())
                .expect("request should build");
            request
                .extensions_mut()
                .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_000))));

            let response = router
                .clone()
                .oneshot(request)
                .await
                .expect("request should complete");
            assert_eq!(
                response.status(),
                http::StatusCode::UNAUTHORIZED,
                "header: {identity_header}, method: {method}, path: {path}"
            );

            let body = response
                .into_body()
                .collect()
                .await
                .expect("body should collect")
                .to_bytes();
            let payload: serde_json::Value =
                serde_json::from_slice(&body).expect("body should be json");
            assert_eq!(
                payload["detail"], "admin authentication required",
                "header: {identity_header}, method: {method}, path: {path}"
            );
        }
    }
}

struct BackupDatabaseFile(std::path::PathBuf);

impl Drop for BackupDatabaseFile {
    fn drop(&mut self) {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", self.0.display()));
        }
    }
}

async fn backup_gateway(
    user_id: &str,
    password: &str,
    encryption_key: &str,
) -> (axum::Router, sqlx::SqlitePool, BackupDatabaseFile) {
    let file = BackupDatabaseFile(
        std::env::temp_dir().join(format!("aether-backup-{}.sqlite", uuid::Uuid::new_v4())),
    );
    let url = format!("sqlite://{}", file.0.display());
    let database = aether_data::SqlDatabaseConfig::new(
        aether_data::DatabaseDriver::Sqlite,
        &url,
        aether_data::SqlPoolConfig {
            min_connections: 0,
            max_connections: 1,
            ..Default::default()
        },
    )
    .unwrap();
    let state = aether_gateway::AppState::new()
        .unwrap()
        .with_data_config_and_background_isolation(
            aether_gateway::GatewayDataConfig::from_database_config(database)
                .with_encryption_key(encryption_key),
            false,
        )
        .unwrap();
    state.run_database_migrations().await.unwrap();
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO users (id, username, password_hash, role, auth_source, created_at, updated_at)
         VALUES (?, ?, ?, 'admin', 'local', 1704067200, 1704067200)",
    )
    .bind(user_id)
    .bind(user_id)
    .bind(bcrypt::hash(password, 4).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    (aether_gateway::build_router_with_state(state), pool, file)
}

async fn backup_request(
    router: &axum::Router,
    method: http::Method,
    path: &str,
    token: Option<&str>,
    payload: Option<&serde_json::Value>,
) -> (http::StatusCode, serde_json::Value) {
    let mut builder = Request::builder()
        .method(method)
        .uri(path)
        .header("content-type", "application/json")
        .header("x-client-device-id", "backup-roundtrip-device")
        .header("user-agent", "AetherBackupTest/1.0");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let mut request = builder
        .body(payload.map_or_else(Body::empty, |payload| Body::from(payload.to_string())))
        .unwrap();
    request
        .extensions_mut()
        .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 40_001))));
    let response = router.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap())
}

async fn backup_login(router: &axum::Router, username: &str, password: &str) -> String {
    let (status, body) = backup_request(
        router,
        http::Method::POST,
        "/api/auth/login",
        None,
        Some(&serde_json::json!({"email": username, "password": password})),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "login: {body}");
    body["access_token"].as_str().unwrap().to_string()
}

#[tokio::test]
async fn current_backups_roundtrip_through_authenticated_sqlite_routes() {
    use aether_crypto::{decrypt_python_fernet_ciphertext, encrypt_python_fernet_plaintext};
    use serde_json::json;
    use sha2::{Digest, Sha256};

    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    let (target, target_pool, _target_file) =
        backup_gateway("target-admin", "target-password", "target-encryption").await;
    for statement in [
        "INSERT INTO providers (id, name, provider_type, billing_type, monthly_quota_usd, monthly_used_usd, quota_reset_day, quota_last_reset_at, quota_expires_at, proxy, created_at, updated_at) VALUES ('source-provider', 'backup-channel', 'custom', 'monthly_quota', 100, 12.5, 30, 1704067200, 4102444800, '{\"node_id\":\"source-manual\"}', 1704067200, 1704067200)",
        "INSERT INTO provider_endpoints (id, provider_id, name, base_url, api_format, api_family, endpoint_kind, created_at, updated_at) VALUES ('source-endpoint', 'source-provider', 'chat', 'https://api.example.com', 'openai:chat', 'openai', 'chat', 1704067200, 1704067200)",
        "INSERT INTO global_models (id, name, display_name, usage_count, created_at, updated_at) VALUES ('source-model', 'backup-model', 'Backup Model', 7, 1704067200, 1704067200)",
        "INSERT INTO models (id, provider_id, global_model_id, provider_model_name, price_per_request, created_at, updated_at) VALUES ('source-mapping', 'source-provider', 'source-model', 'upstream-model', 0.02, 1704067200, 1704067200)",
        "INSERT INTO proxy_nodes (id, name, ip, port, is_manual, proxy_url, proxy_username, proxy_password, created_at, updated_at) VALUES ('source-manual', 'manual-proxy', 'proxy.example.com', 8080, 1, 'http://proxy.example.com:8080', 'proxy-user', 'proxy-password', 1704067200, 1704067200)",
        "INSERT INTO proxy_nodes (id, name, ip, port, is_manual, tunnel_mode, status, remote_config, created_at, updated_at) VALUES ('source-tunnel', 'tunnel-proxy', '127.0.0.1', 0, 0, 1, 'offline', '{\"log_level\":\"info\"}', 1704067200, 1704067200)",
        "UPDATE users SET allowed_providers = '[\"source-provider\"]', allowed_providers_mode = 'specific' WHERE id = 'source-admin'",
        "INSERT INTO user_preferences (id, user_id, bio, default_provider_id, theme, created_at, updated_at) VALUES ('source-preferences', 'source-admin', 'backup profile', 'source-provider', 'dark', 1704067200, 1704067200)",
        "INSERT INTO stats_user_daily (id, user_id, date, total_requests, success_requests, input_tokens, output_tokens, total_cost, created_at, updated_at) VALUES ('source-user-day', 'source-admin', 1704067200, 12, 12, 90, 60, 0.25, 1704067200, 1704067200)",
        "INSERT INTO stats_daily_api_key (id, api_key_id, date, total_requests, success_requests, input_tokens, output_tokens, total_cost, created_at, updated_at) VALUES ('source-key-day', 'source-user-key', 1704067200, 12, 12, 90, 60, 0.25, 1704067200, 1704067200)",
    ] {
        sqlx::query(statement).execute(&source_pool).await.unwrap();
    }
    for (id, auth_type, secret, auth_config) in [
        ("channel-key", "api_key", Some("channel-secret"), None),
        (
            "oauth-a",
            "oauth",
            Some("access-a"),
            Some(json!({"refresh_token": "refresh-a"})),
        ),
        ("oauth-b", "oauth", Some("access-b"), None),
        (
            "service-account",
            "service_account",
            None,
            Some(json!({"client_email": "backup@example.com", "private_key": "test-private-key"})),
        ),
    ] {
        sqlx::query(
            "INSERT INTO provider_api_keys (id, provider_id, name, auth_type, api_key, auth_config, api_formats, concurrent_limit, expires_at, created_at, updated_at)
             VALUES (?, 'source-provider', 'shared-name', ?, ?, ?, '[\"openai:chat\"]', 7, 4102444800, 1704067200, 1704067200)",
        )
        .bind(id)
        .bind(auth_type)
        .bind(secret.map(|secret| encrypt_python_fernet_plaintext("source-encryption", secret).unwrap()))
        .bind(auth_config.map(|config| encrypt_python_fernet_plaintext("source-encryption", &config.to_string()).unwrap()))
        .execute(&source_pool)
        .await
        .unwrap();
    }
    for (id, secret, standalone) in [
        ("source-user-key", "sk-backup-user", false),
        ("source-standalone", "sk-backup-hash-only", true),
    ] {
        sqlx::query(
            "INSERT INTO api_keys (id, user_id, key_hash, key_encrypted, name, is_standalone, is_active, rate_limit, concurrent_limit, total_requests, total_tokens, total_cost_usd, created_at, updated_at)
             VALUES (?, 'source-admin', ?, ?, ?, ?, ?, ?, 3, 12, 150, 0.25, 1704067200, 1704067200)",
        )
        .bind(id)
        .bind(format!("{:x}", Sha256::digest(secret.as_bytes())))
        .bind((!standalone).then(|| encrypt_python_fernet_plaintext("source-encryption", secret).unwrap()))
        .bind(id)
        .bind(standalone)
        .bind(!standalone)
        .bind(standalone.then_some(0))
        .execute(&source_pool)
        .await
        .unwrap();
    }
    for (key, value) in [
        (
            "smtp_password",
            json!(encrypt_python_fernet_plaintext("source-encryption", "smtp-secret").unwrap()),
        ),
        ("external_models_proxy_node_id", json!("source-manual")),
    ] {
        sqlx::query("INSERT INTO system_configs (id, key, value, created_at, updated_at) VALUES (?, ?, ?, 1704067200, 1704067200)")
            .bind(key).bind(key).bind(value.to_string()).execute(&source_pool).await.unwrap();
    }

    let source_token = backup_login(&source, "source-admin", "source-password").await;
    let target_token = backup_login(&target, "target-admin", "target-password").await;
    let (status, backup) = backup_request(
        &source,
        http::Method::GET,
        "/api/admin/system/data/export",
        Some(&source_token),
        None,
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "export: {backup}");
    assert_eq!(backup["version"], "2.0");
    assert_eq!(backup["config_data"]["version"], "3.0");
    assert_eq!(backup["user_data"]["version"], "2.0");
    assert_eq!(
        backup["config_data"]["providers"][0]["api_keys"]
            .as_array()
            .unwrap()
            .len(),
        4
    );

    let mut invalid = backup.clone();
    invalid["user_data"]["users"][0]["password_hash"] = json!("broken");
    let (status, _) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/data/import",
        Some(&target_token),
        Some(&invalid),
    )
    .await;
    assert_eq!(status, http::StatusCode::BAD_REQUEST);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM providers")
            .fetch_one(&target_pool)
            .await
            .unwrap(),
        0
    );

    let mut request = backup.clone();
    request["merge_mode"] = json!("skip");
    let (status, skipped) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/data/import",
        Some(&target_token),
        Some(&request),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "skip import: {skipped}");
    assert_eq!(skipped["users"]["stats"]["users"]["skipped"], 1);
    assert_eq!(skipped["users"]["stats"]["api_keys"]["created"], 1);
    sqlx::query("UPDATE api_keys SET rate_limit = 99, concurrent_limit = 99, is_active = 0 WHERE is_standalone = 0")
        .execute(&target_pool).await.unwrap();
    request["merge_mode"] = json!("overwrite");
    let (status, imported) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/data/import",
        Some(&target_token),
        Some(&request),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "overwrite import: {imported}");
    assert_eq!(imported["config"]["stats"]["keys"]["updated"], 4);
    assert_eq!(imported["users"]["reauthentication_required"], true);
    assert_eq!(
        backup_request(
            &target,
            http::Method::GET,
            "/api/admin/system/data/export",
            Some(&target_token),
            None
        )
        .await
        .0,
        http::StatusCode::UNAUTHORIZED
    );
    let target_token = backup_login(&target, "source-admin", "source-password").await;
    let (status, restored) = backup_request(
        &target,
        http::Method::GET,
        "/api/admin/system/data/export",
        Some(&target_token),
        None,
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "restored export: {restored}");
    let mut expected_provider = backup["config_data"]["providers"][0].clone();
    expected_provider["id"] = restored["config_data"]["providers"][0]["id"].clone();
    assert_eq!(restored["config_data"]["providers"][0], expected_provider);
    assert_eq!(
        restored["config_data"]["global_models"],
        backup["config_data"]["global_models"]
    );
    assert_eq!(
        restored["config_data"]["proxy_nodes"],
        backup["config_data"]["proxy_nodes"]
    );
    let user = &restored["user_data"]["users"][0];
    assert_eq!(user["id"], "target-admin");
    assert_eq!(user["username"], "source-admin");
    assert_eq!(user["allowed_providers"], json!([expected_provider["id"]]));
    assert_eq!(
        user["preferences"]["default_provider_id"],
        expected_provider["id"]
    );
    assert_eq!(user["preferences"]["theme"], "dark");
    assert_eq!(user["preferences"]["bio"], "backup profile");
    assert_eq!(user["request_count"], 12);
    assert_eq!(user["total_tokens"], 150);
    assert_eq!(user["api_keys"][0]["key"], "sk-backup-user");
    assert_eq!(user["api_keys"][0]["rate_limit"], serde_json::Value::Null);
    assert_eq!(user["api_keys"][0]["concurrent_limit"], 3);
    assert_eq!(user["api_keys"][0]["is_active"], true);
    assert_eq!(
        restored["user_data"]["standalone_keys"][0]["key"],
        serde_json::Value::Null
    );
    assert_eq!(restored["user_data"]["standalone_keys"][0]["rate_limit"], 0);
    assert_eq!(
        restored["user_data"]["usage_aggregates"]["stats_daily_api_key"][0]["total_requests"],
        12
    );
    let encrypted: String =
        sqlx::query_scalar("SELECT key_encrypted FROM api_keys WHERE is_standalone = 0")
            .fetch_one(&target_pool)
            .await
            .unwrap();
    assert_eq!(
        decrypt_python_fernet_ciphertext("target-encryption", &encrypted).unwrap(),
        "sk-backup-user"
    );
    assert!(decrypt_python_fernet_ciphertext("source-encryption", &encrypted).is_err());

    let (status, mut config) = backup_request(
        &source,
        http::Method::GET,
        "/api/admin/system/config/export",
        Some(&source_token),
        None,
    )
    .await;
    assert_eq!(status, http::StatusCode::OK);
    for mode in ["skip", "overwrite"] {
        config["merge_mode"] = json!(mode);
        let (status, result) = backup_request(
            &target,
            http::Method::POST,
            "/api/admin/system/config/import",
            Some(&target_token),
            Some(&config),
        )
        .await;
        assert_eq!(status, http::StatusCode::OK, "config {mode}: {result}");
        assert_eq!(result["stats"]["keys"]["created"], 0);
        assert_eq!(result["stats"]["errors"], json!([]));
    }
    request["merge_mode"] = json!("skip");
    let (status, repeated) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/data/import",
        Some(&target_token),
        Some(&request),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "repeat import: {repeated}");
    assert_eq!(repeated["users"]["stats"]["api_keys"]["created"], 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM api_keys")
            .fetch_one(&target_pool)
            .await
            .unwrap(),
        2
    );
    source_pool.close().await;
    target_pool.close().await;
}
