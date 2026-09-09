use std::{collections::BTreeMap, net::SocketAddr};

use aether_gateway::build_router;
use axum::body::Body;
use axum::extract::ConnectInfo;
use http::Request;
use http_body_util::BodyExt;
use serde_json::{json, Value};
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

const CONFIG_BACKUP_TABLES: &[&str] = &[
    "global_models",
    "providers",
    "provider_endpoints",
    "provider_api_keys",
    "models",
    "proxy_nodes",
    "routing_groups",
    "routing_group_versions",
    "routing_group_bindings",
    "system_configs",
];

const USER_BACKUP_TABLES: &[&str] = &[
    "users",
    "user_preferences",
    "api_keys",
    "stats_daily",
    "stats_user_daily",
    "stats_daily_api_key",
    "user_sessions",
];

async fn backup_table_snapshot(
    pool: &sqlx::SqlitePool,
    tables: &[&str],
) -> BTreeMap<String, Vec<String>> {
    let mut snapshot = BTreeMap::new();
    for table in tables {
        let mut columns: Vec<String> =
            sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                .bind(*table)
                .fetch_all(pool)
                .await
                .unwrap();
        assert!(!columns.is_empty(), "missing migrated table {table}");
        // Authentication may touch activity timestamps independently of the import.
        // Session identities, tokens, expiry and revocation fields remain checked.
        if *table == "user_sessions" {
            columns.retain(|column| !matches!(column.as_str(), "last_seen_at" | "updated_at"));
        }
        let projection = columns
            .iter()
            .map(|column| format!("quote(\"{}\")", column.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(", ");
        let mut rows: Vec<String> =
            sqlx::query_scalar(&format!("SELECT json_array({projection}) FROM \"{table}\""))
                .fetch_all(pool)
                .await
                .unwrap();
        rows.sort_unstable();
        snapshot.insert((*table).to_string(), rows);
    }
    snapshot
}

fn assert_versionless_backup(backup: &Value) {
    assert!(backup.get("version").is_none(), "unexpected backup version");
    for field in ["config_data", "user_data"] {
        if let Some(part) = backup.get(field) {
            assert!(part.get("version").is_none(), "unexpected {field} version");
        }
    }
}

fn config_backup_content(config: &Value) -> Value {
    let mut content = config.clone();
    let root = content.as_object_mut().unwrap();
    for field in ["exported_at", "version", "merge_mode"] {
        root.remove(field);
    }
    for provider in root.get_mut("providers").unwrap().as_array_mut().unwrap() {
        let provider = provider.as_object_mut().unwrap();
        provider.remove("id");
        for field in ["api_keys", "endpoints", "models"] {
            let rows = provider.get_mut(field).unwrap().as_array_mut().unwrap();
            for row in rows.iter_mut() {
                row.as_object_mut().unwrap().remove("id");
            }
            rows.sort_by_key(Value::to_string);
        }
    }
    for field in [
        "providers",
        "global_models",
        "proxy_nodes",
        "system_configs",
    ] {
        root.get_mut(field)
            .unwrap()
            .as_array_mut()
            .unwrap()
            .sort_by_key(Value::to_string);
    }
    if let Some(strategy) = root
        .get_mut("routing_strategy")
        .and_then(Value::as_object_mut)
    {
        // Import preserves the strategy content but records a target-local revision.
        for field in ["id", "version", "published_at"] {
            strategy.remove(field);
        }
    }
    content
}

async fn export_backup(router: &axum::Router, token: &str, scope: &str) -> Value {
    let (status, backup) = backup_request(
        router,
        http::Method::GET,
        &format!("/api/admin/system/{scope}/export"),
        Some(token),
        None,
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "{scope} export: {backup}");
    assert_versionless_backup(&backup);
    backup
}

async fn seed_backup_source(pool: &sqlx::SqlitePool) {
    use aether_crypto::encrypt_python_fernet_plaintext;
    use sha2::{Digest, Sha256};

    for statement in [
        "INSERT INTO providers (id, name, provider_type, billing_type, monthly_quota_usd, monthly_used_usd, quota_reset_day, quota_last_reset_at, quota_expires_at, proxy, created_at, updated_at) VALUES ('source-provider', 'backup-channel', 'custom', 'monthly_quota', 100, 12.5, 30, 1704067200, 4102444800, '{\"node_id\":\"source-manual\"}', 1704067200, 1704067200)",
        "INSERT INTO provider_endpoints (id, provider_id, name, base_url, api_format, api_family, endpoint_kind, created_at, updated_at) VALUES ('source-endpoint', 'source-provider', 'chat', 'https://api.example.com', 'openai:chat', 'openai', 'chat', 1704067200, 1704067200)",
        "INSERT INTO global_models (id, name, display_name, usage_count, created_at, updated_at) VALUES ('source-model', 'backup-model', 'Backup Model', 7, 1704067200, 1704067200)",
        "INSERT INTO models (id, provider_id, global_model_id, provider_model_name, price_per_request, created_at, updated_at) VALUES ('source-mapping', 'source-provider', 'source-model', 'upstream-model', 0.02, 1704067200, 1704067200)",
        "INSERT INTO proxy_nodes (id, name, ip, port, is_manual, proxy_url, proxy_username, proxy_password, created_at, updated_at) VALUES ('source-manual', 'manual-proxy', 'proxy.example.com', 8080, 1, 'http://proxy.example.com:8080', 'proxy-user', 'proxy-password', 1704067200, 1704067200)",
        "INSERT INTO proxy_nodes (id, name, ip, port, is_manual, tunnel_mode, status, remote_config, created_at, updated_at) VALUES ('source-tunnel', 'tunnel-proxy', '127.0.0.1', 0, 0, 1, 'offline', '{\"log_level\":\"info\"}', 1704067200, 1704067200)",
        "UPDATE users SET allowed_providers = '[\"source-provider\"]', allowed_providers_mode = 'specific' WHERE id = 'source-admin'",
        "INSERT INTO user_preferences (id, user_id, bio, default_provider_id, theme, created_at, updated_at) VALUES ('source-preferences', 'source-admin', 'backup profile', 'source-provider', 'dark', 1704067200, 1704067200)",
        "INSERT INTO stats_daily (id, date, total_requests, success_requests, input_tokens, output_tokens, total_cost, created_at, updated_at) VALUES ('source-day', 1704067200, 12, 12, 90, 60, 0.25, 1704067200, 1704067200)",
        "INSERT INTO stats_user_daily (id, user_id, date, total_requests, success_requests, input_tokens, output_tokens, total_cost, created_at, updated_at) VALUES ('source-user-day', 'source-admin', 1704067200, 12, 12, 90, 60, 0.25, 1704067200, 1704067200)",
        "INSERT INTO stats_daily_api_key (id, api_key_id, date, total_requests, success_requests, input_tokens, output_tokens, total_cost, created_at, updated_at) VALUES ('source-key-day', 'source-user-key', 1704067200, 12, 12, 90, 60, 0.25, 1704067200, 1704067200)",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
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
        .execute(pool)
        .await
        .unwrap();
    }
    for (id, secret, standalone) in [
        ("source-user-key", "sk-backup-user", false),
        ("source-standalone", "sk-backup-hash-only", true),
    ] {
        sqlx::query(
            "INSERT INTO api_keys (id, user_id, key_hash, key_encrypted, name, is_standalone, is_active, is_locked, rate_limit, concurrent_limit, total_requests, total_tokens, total_cost_usd, created_at, updated_at)
             VALUES (?, 'source-admin', ?, ?, ?, ?, ?, ?, ?, 3, 12, 150, 0.25, 1704067200, 1704067200)",
        )
        .bind(id)
        .bind(format!("{:x}", Sha256::digest(secret.as_bytes())))
        .bind((!standalone).then(|| encrypt_python_fernet_plaintext("source-encryption", secret).unwrap()))
        .bind(id)
        .bind(standalone)
        .bind(!standalone)
        .bind(!standalone)
        .bind(standalone.then_some(0))
        .execute(pool)
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
            .bind(key).bind(key).bind(value.to_string()).execute(pool).await.unwrap();
    }
    let routing_config =
        serde_json::to_string(&aether_routing_core::RoutingGroupConfig::default()).unwrap();
    sqlx::query("INSERT INTO routing_groups (id, name, enabled, is_system_default, config_json, version, published_at, created_at, updated_at) VALUES ('source-routing', 'backup-routing', 1, 1, ?, 1, 1704067200, 1704067200, 1704067200)")
        .bind(&routing_config).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO routing_group_versions (id, group_id, version, config_json, created_at) VALUES ('source-routing-version', 'source-routing', 1, ?, 1704067200)")
        .bind(routing_config).execute(pool).await.unwrap();
}

async fn seed_backup_target(pool: &sqlx::SqlitePool) {
    use aether_crypto::encrypt_python_fernet_plaintext;
    use sha2::{Digest, Sha256};

    for statement in [
        "INSERT INTO providers (id, name, provider_type, billing_type, monthly_quota_usd, monthly_used_usd, created_at, updated_at) VALUES ('target-provider', 'backup-channel', 'custom', 'monthly_quota', 9, 1, 1704067200, 1704067200)",
        "INSERT INTO global_models (id, name, display_name, usage_count, created_at, updated_at) VALUES ('target-model', 'backup-model', 'Target Model', 1, 1704067200, 1704067200)",
        "INSERT INTO stats_daily (id, date, total_requests, success_requests, input_tokens, output_tokens, created_at, updated_at) VALUES ('target-day', 1704067200, 1, 1, 1, 1, 1704067200, 1704067200)",
        "INSERT INTO stats_user_daily (id, user_id, date, total_requests, success_requests, input_tokens, output_tokens, created_at, updated_at) VALUES ('target-user-day', 'target-admin', 1704067200, 1, 1, 1, 1, 1704067200, 1704067200)",
        "INSERT INTO stats_daily_api_key (id, api_key_id, date, total_requests, success_requests, input_tokens, output_tokens, created_at, updated_at) VALUES ('target-key-day', 'target-user-key', 1704067200, 1, 1, 1, 1, 1704067200, 1704067200)",
    ] {
        sqlx::query(statement).execute(pool).await.unwrap();
    }
    for (key, value) in [
        ("site_name", json!("Target site")),
        ("external_models_proxy_node_id", Value::Null),
        (
            "smtp_password",
            json!(encrypt_python_fernet_plaintext("target-encryption", "target-smtp").unwrap()),
        ),
    ] {
        sqlx::query("INSERT INTO system_configs (id, key, value, created_at, updated_at) VALUES (?, ?, ?, 1704067200, 1704067200)")
            .bind(key).bind(key).bind(value.to_string()).execute(pool).await.unwrap();
    }
    sqlx::query("INSERT INTO api_keys (id, user_id, key_hash, key_encrypted, name, is_standalone, is_active, rate_limit, concurrent_limit, total_requests, total_tokens, total_cost_usd, created_at, updated_at) VALUES ('target-user-key', 'target-admin', ?, ?, 'target-user-key', 0, 1, 99, 99, 1, 2, 0.01, 1704067200, 1704067200)")
        .bind(format!("{:x}", Sha256::digest(b"sk-backup-user")))
        .bind(encrypt_python_fernet_plaintext("target-encryption", "sk-backup-user").unwrap())
        .execute(pool).await.unwrap();
}

#[tokio::test]
async fn backups_roundtrip_through_authenticated_sqlite_routes() {
    use aether_crypto::decrypt_python_fernet_ciphertext;

    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    let (target, target_pool, _target_file) =
        backup_gateway("target-admin", "target-password", "target-encryption").await;
    seed_backup_source(&source_pool).await;

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
    assert_versionless_backup(&backup);
    assert_eq!(
        backup["user_data"]["users"][0]["api_keys"][0]["is_locked"],
        true
    );
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
    sqlx::query("UPDATE api_keys SET rate_limit = 99, concurrent_limit = 99, is_active = 0, is_locked = 0 WHERE is_standalone = 0")
        .execute(&target_pool).await.unwrap();
    request["version"] = json!("1.0");
    request["config_data"]["version"] = json!("2.3");
    request["user_data"]["version"] = json!("1.5");
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
    assert_versionless_backup(&restored);
    assert_eq!(
        config_backup_content(&restored["config_data"]),
        config_backup_content(&backup["config_data"])
    );
    let mut expected_provider = backup["config_data"]["providers"][0].clone();
    expected_provider["id"] = restored["config_data"]["providers"][0]["id"].clone();
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
    assert_eq!(user["api_keys"][0]["is_locked"], true);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT is_locked FROM api_keys WHERE is_standalone = 0")
            .fetch_one(&target_pool)
            .await
            .unwrap(),
        1
    );
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
    assert_versionless_backup(&config);
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
    request["version"] = json!({"unrecognized": "metadata"});
    request["config_data"]["version"] = json!(12345);
    request["user_data"]["version"] = Value::Null;
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

#[tokio::test]
async fn backup_usage_aggregates_roundtrip_preserves_zero_rows_and_completion_state() {
    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    let (target, target_pool, _target_file) =
        backup_gateway("target-admin", "target-password", "target-encryption").await;
    seed_backup_source(&source_pool).await;
    seed_backup_target(&target_pool).await;
    for statement in [
        "INSERT INTO stats_daily (id, date, created_at, updated_at) VALUES ('source-zero-day', 1704153600, 1704153600, 1704153600)",
        "INSERT INTO stats_user_daily (id, user_id, date, created_at, updated_at) VALUES ('source-zero-user-day', 'source-admin', 1704153600, 1704153600, 1704153600)",
        "INSERT INTO stats_daily_api_key (id, api_key_id, date, created_at, updated_at) VALUES ('source-zero-key-day', 'source-user-key', 1704153600, 1704153600, 1704153600)",
        "INSERT INTO stats_daily (id, date, actual_total_cost, is_complete, aggregated_at, created_at, updated_at) VALUES ('source-actual-cost-day', 1704240000, 1.75, 1, 1704326400, 1704240000, 1704240000)",
        "UPDATE stats_user_daily SET username = 'recorded-admin-name' WHERE id = 'source-user-day'",
        "UPDATE stats_daily_api_key SET api_key_name = 'recorded-key-name' WHERE id = 'source-key-day'",
    ] {
        sqlx::query(statement).execute(&source_pool).await.unwrap();
    }
    // Overwriting an existing completed day must restore the source's false value.
    sqlx::query("UPDATE stats_daily SET is_complete = 1 WHERE id = 'target-day'")
        .execute(&target_pool)
        .await
        .unwrap();
    let source_token = backup_login(&source, "source-admin", "source-password").await;
    let target_token = backup_login(&target, "target-admin", "target-password").await;
    let mut request = export_backup(&source, &source_token, "data").await;
    let mut expected = request["user_data"]["usage_aggregates"].clone();
    for (table, count) in [
        ("stats_daily", 3),
        ("stats_user_daily", 2),
        ("stats_daily_api_key", 2),
    ] {
        let rows = expected[table].as_array().unwrap();
        assert_eq!(rows.len(), count, "source export omitted {table} rows");
        let zero_day = rows
            .iter()
            .find(|row| row["date_unix_secs"] == 1_704_153_600u64)
            .unwrap();
        for field in [
            "total_requests",
            "success_requests",
            "error_requests",
            "input_tokens",
            "output_tokens",
            "cache_creation_tokens",
            "cache_read_tokens",
        ] {
            assert_eq!(zero_day[field], 0, "{table}.{field}");
        }
        assert_eq!(zero_day["total_cost"], 0.0, "{table}.total_cost");
    }
    assert_eq!(expected["stats_daily"][0]["total_requests"], 12);
    assert_eq!(expected["stats_daily"][0]["is_complete"], false);
    assert_eq!(expected["stats_daily"][1]["actual_total_cost"], 0.0);
    assert_eq!(expected["stats_daily"][1]["is_complete"], false);
    assert_eq!(expected["stats_daily"][2]["total_requests"], 0);
    assert_eq!(expected["stats_daily"][2]["total_cost"], 0.0);
    assert_eq!(expected["stats_daily"][2]["actual_total_cost"], 1.75);
    assert_eq!(expected["stats_daily"][2]["is_complete"], true);

    request["merge_mode"] = json!("overwrite");
    let (status, response) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/data/import",
        Some(&target_token),
        Some(&request),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "stats import: {response}");
    let target_token = backup_login(&target, "source-admin", "source-password").await;
    let restored = export_backup(&target, &target_token, "data").await;
    let restored_user = &restored["user_data"]["users"][0];
    assert_eq!(restored_user["id"], "target-admin");
    assert_eq!(
        restored_user["api_keys"][0]["api_key_id"],
        "target-user-key"
    );
    for row in expected["stats_user_daily"].as_array_mut().unwrap() {
        row["user_id"] = restored_user["id"].clone();
    }
    for row in expected["stats_daily_api_key"].as_array_mut().unwrap() {
        row["api_key_id"] = restored_user["api_keys"][0]["api_key_id"].clone();
    }
    assert_eq!(restored["user_data"]["usage_aggregates"], expected);
    source_pool.close().await;
    target_pool.close().await;
}

#[tokio::test]
async fn config_backups_roundtrip_between_instances_without_version_gates() {
    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    let (target, target_pool, _target_file) =
        backup_gateway("target-admin", "target-password", "target-encryption").await;
    seed_backup_source(&source_pool).await;
    let source_token = backup_login(&source, "source-admin", "source-password").await;
    let target_token = backup_login(&target, "target-admin", "target-password").await;
    let config = export_backup(&source, &source_token, "config").await;
    let original_admin = backup_table_snapshot(&target_pool, &["users", "user_preferences"]).await;

    for version in [
        None,
        Some(json!("2.3")),
        Some(json!({"arbitrary": [9, 8, 7]})),
    ] {
        let mut request = config.clone();
        request["merge_mode"] = json!("overwrite");
        if let Some(version) = version {
            request["version"] = version;
        }
        let (status, response) = backup_request(
            &target,
            http::Method::POST,
            "/api/admin/system/config/import",
            Some(&target_token),
            Some(&request),
        )
        .await;
        assert_eq!(status, http::StatusCode::OK, "config import: {response}");
        let restored = export_backup(&target, &target_token, "config").await;
        assert_eq!(
            config_backup_content(&restored),
            config_backup_content(&config)
        );
        assert_eq!(
            backup_table_snapshot(&target_pool, &["users", "user_preferences"]).await,
            original_admin,
            "configuration-only import must retain the target administrator"
        );
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM api_keys")
            .fetch_one(&target_pool)
            .await
            .unwrap(),
        0
    );
    source_pool.close().await;
    target_pool.close().await;
}

#[tokio::test]
async fn config_backup_late_sql_failure_rolls_back_every_configuration_table() {
    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    let (target, target_pool, _target_file) =
        backup_gateway("target-admin", "target-password", "target-encryption").await;
    seed_backup_source(&source_pool).await;
    seed_backup_target(&target_pool).await;
    let source_token = backup_login(&source, "source-admin", "source-password").await;
    let target_token = backup_login(&target, "target-admin", "target-password").await;
    let mut request = export_backup(&source, &source_token, "config").await;
    request["merge_mode"] = json!("overwrite");
    let before = backup_table_snapshot(&target_pool, CONFIG_BACKUP_TABLES).await;

    // These conditions require earlier creates and overwrites to be visible to
    // the same connection before the final system setting is deliberately rejected.
    for operation in ["INSERT", "UPDATE"] {
        sqlx::query(&format!(
            "CREATE TRIGGER fail_config_{operation} BEFORE {operation} ON system_configs
             WHEN NEW.key = 'smtp_password'
               AND EXISTS (SELECT 1 FROM global_models WHERE name = 'backup-model' AND display_name = 'Backup Model')
               AND EXISTS (SELECT 1 FROM providers WHERE name = 'backup-channel' AND monthly_quota_usd = 100)
               AND (SELECT COUNT(*) FROM provider_api_keys) = 4
               AND EXISTS (SELECT 1 FROM provider_endpoints WHERE base_url LIKE 'https://api.example.com%')
               AND EXISTS (SELECT 1 FROM models WHERE provider_model_name = 'upstream-model' AND price_per_request = 0.02)
               AND EXISTS (SELECT 1 FROM proxy_nodes WHERE name = 'tunnel-proxy')
               AND EXISTS (SELECT 1 FROM routing_groups WHERE name = 'backup-routing')
               AND EXISTS (SELECT 1 FROM routing_group_versions)
               AND EXISTS (SELECT 1 FROM system_configs WHERE key = 'external_models_proxy_node_id' AND value <> 'null')
             BEGIN SELECT RAISE(ABORT, 'forced late configuration import failure'); END"
        ))
        .execute(&target_pool)
        .await
        .unwrap();
    }
    let (status, response) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/config/import",
        Some(&target_token),
        Some(&request),
    )
    .await;
    assert!(
        !status.is_success(),
        "expected injected failure: {response}"
    );
    assert!(
        response
            .to_string()
            .contains("forced late configuration import failure"),
        "{response}"
    );
    assert_eq!(
        backup_table_snapshot(&target_pool, CONFIG_BACKUP_TABLES).await,
        before,
        "all configuration rows and values must roll back together"
    );

    for operation in ["INSERT", "UPDATE"] {
        sqlx::query(&format!("DROP TRIGGER fail_config_{operation}"))
            .execute(&target_pool)
            .await
            .unwrap();
    }
    let (status, response) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/config/import",
        Some(&target_token),
        Some(&request),
    )
    .await;
    assert_eq!(
        status,
        http::StatusCode::OK,
        "retry after removing trigger: {response}"
    );
    assert_ne!(
        backup_table_snapshot(&target_pool, CONFIG_BACKUP_TABLES).await,
        before
    );
    let restored = export_backup(&target, &target_token, "config").await;
    assert_eq!(restored["providers"][0]["monthly_quota_usd"], 100.0);
    source_pool.close().await;
    target_pool.close().await;
}

#[tokio::test]
async fn full_backup_preference_failure_rolls_back_data_password_and_sessions() {
    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    seed_backup_source(&source_pool).await;
    let source_token = backup_login(&source, "source-admin", "source-password").await;
    let mut request = export_backup(&source, &source_token, "data").await;
    request["merge_mode"] = json!("overwrite");
    let tables: Vec<&str> = CONFIG_BACKUP_TABLES
        .iter()
        .chain(USER_BACKUP_TABLES)
        .copied()
        .collect();

    for operation in ["INSERT", "UPDATE"] {
        let (target, target_pool, _target_file) =
            backup_gateway("target-admin", "target-password", "target-encryption").await;
        seed_backup_target(&target_pool).await;
        if operation == "UPDATE" {
            sqlx::query("INSERT INTO user_preferences (id, user_id, bio, theme, created_at, updated_at) VALUES ('target-preferences', 'target-admin', 'retain target profile', 'light', 1704067200, 1704067200)")
                .execute(&target_pool).await.unwrap();
        }
        let target_token = backup_login(&target, "target-admin", "target-password").await;
        let before = backup_table_snapshot(&target_pool, &tables).await;
        sqlx::query(&format!(
            "CREATE TRIGGER fail_backup_preferences BEFORE {operation} ON user_preferences
             WHEN NEW.user_id = 'target-admin'
               AND (SELECT COUNT(*) FROM provider_api_keys) = 4
               AND EXISTS (SELECT 1 FROM providers WHERE name = 'backup-channel' AND monthly_quota_usd = 100)
               AND (SELECT COUNT(*) FROM api_keys) = 2
               AND EXISTS (SELECT 1 FROM api_keys WHERE id = 'target-user-key' AND total_requests = 12)
               AND EXISTS (SELECT 1 FROM stats_daily WHERE date = 1704067200 AND total_requests = 12)
               AND EXISTS (SELECT 1 FROM stats_user_daily WHERE user_id = 'target-admin' AND total_requests = 12)
               AND EXISTS (SELECT 1 FROM stats_daily_api_key WHERE api_key_id = 'target-user-key' AND total_requests = 12)
               AND EXISTS (SELECT 1 FROM users WHERE id = 'target-admin' AND username = 'source-admin')
               AND EXISTS (SELECT 1 FROM user_sessions WHERE user_id = 'target-admin' AND revoked_at IS NOT NULL AND revoke_reason = 'backup_restored')
             BEGIN SELECT RAISE(ABORT, 'forced late preference import failure'); END"
        ))
        .execute(&target_pool)
        .await
        .unwrap();

        let (status, response) = backup_request(
            &target,
            http::Method::POST,
            "/api/admin/system/data/import",
            Some(&target_token),
            Some(&request),
        )
        .await;
        assert!(
            !status.is_success(),
            "expected {operation} failure: {response}"
        );
        assert!(
            response
                .to_string()
                .contains("forced late preference import failure"),
            "{response}"
        );
        assert_eq!(
            backup_table_snapshot(&target_pool, &tables).await,
            before,
            "{operation} failure must restore data, password and session revocation state"
        );
        let hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE id = 'target-admin'")
                .fetch_one(&target_pool)
                .await
                .unwrap();
        assert!(bcrypt::verify("target-password", &hash).unwrap());
        assert!(!bcrypt::verify("source-password", &hash).unwrap());
        let unchanged = export_backup(&target, &target_token, "data").await;
        assert_eq!(
            unchanged["user_data"]["users"][0]["username"],
            "target-admin"
        );
        let retry_token = backup_login(&target, "target-admin", "target-password").await;

        sqlx::query("DROP TRIGGER fail_backup_preferences")
            .execute(&target_pool)
            .await
            .unwrap();
        let (status, response) = backup_request(
            &target,
            http::Method::POST,
            "/api/admin/system/data/import",
            Some(&retry_token),
            Some(&request),
        )
        .await;
        assert_eq!(
            status,
            http::StatusCode::OK,
            "valid backup retries successfully: {response}"
        );
        assert_eq!(response["users"]["reauthentication_required"], true);
        let (status, _) = backup_request(
            &target,
            http::Method::GET,
            "/api/admin/system/data/export",
            Some(&target_token),
            None,
        )
        .await;
        assert_eq!(status, http::StatusCode::UNAUTHORIZED);
        let restored_token = backup_login(&target, "source-admin", "source-password").await;
        let restored = export_backup(&target, &restored_token, "data").await;
        assert_eq!(
            restored["user_data"]["users"][0]["preferences"]["bio"],
            "backup profile"
        );
        assert_eq!(
            restored["user_data"]["usage_aggregates"]["stats_daily"][0]["total_requests"],
            12
        );
        target_pool.close().await;
    }
    source_pool.close().await;
}

#[tokio::test]
async fn backup_exports_fail_without_partial_payload_when_credentials_cannot_decrypt() {
    use aether_crypto::encrypt_python_fernet_plaintext;

    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    seed_backup_source(&source_pool).await;
    let token = backup_login(&source, "source-admin", "source-password").await;
    export_backup(&source, &token, "data").await;
    let unreadable =
        encrypt_python_fernet_plaintext("unavailable-source-encryption", "synthetic-secret")
            .unwrap();

    for (label, select, update, config_credential, json_value) in [
        (
            "channel API key",
            "SELECT api_key FROM provider_api_keys WHERE id = 'channel-key'",
            "UPDATE provider_api_keys SET api_key = ? WHERE id = 'channel-key'",
            true,
            false,
        ),
        (
            "OAuth auth config",
            "SELECT auth_config FROM provider_api_keys WHERE id = 'oauth-a'",
            "UPDATE provider_api_keys SET auth_config = ? WHERE id = 'oauth-a'",
            true,
            false,
        ),
        (
            "SMTP password",
            "SELECT value FROM system_configs WHERE key = 'smtp_password'",
            "UPDATE system_configs SET value = ? WHERE key = 'smtp_password'",
            true,
            true,
        ),
        (
            "user API key",
            "SELECT key_encrypted FROM api_keys WHERE id = 'source-user-key'",
            "UPDATE api_keys SET key_encrypted = ? WHERE id = 'source-user-key'",
            false,
            false,
        ),
    ] {
        let original: String = sqlx::query_scalar(select)
            .fetch_one(&source_pool)
            .await
            .unwrap();
        let corrupted = if json_value {
            json!(unreadable).to_string()
        } else {
            unreadable.clone()
        };
        sqlx::query(update)
            .bind(corrupted)
            .execute(&source_pool)
            .await
            .unwrap();
        let scopes: &[&str] = if config_credential {
            &["config", "data"]
        } else {
            &["data"]
        };
        for scope in scopes {
            let (status, body) = backup_request(
                &source,
                http::Method::GET,
                &format!("/api/admin/system/{scope}/export"),
                Some(&token),
                None,
            )
            .await;
            assert!(
                !status.is_success(),
                "{label} must fail {scope} export: {body}"
            );
            for field in [
                "exported_at",
                "config_data",
                "user_data",
                "providers",
                "global_models",
                "users",
                "standalone_keys",
            ] {
                assert!(
                    body.get(field).is_none(),
                    "{label} leaked partial {field}: {body}"
                );
            }
        }
        sqlx::query(update)
            .bind(original)
            .execute(&source_pool)
            .await
            .unwrap();
        export_backup(&source, &token, "data").await;
    }
    source_pool.close().await;
}

#[tokio::test]
async fn backup_exports_fail_without_partial_payload_when_string_lists_are_corrupt() {
    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    seed_backup_source(&source_pool).await;
    let token = backup_login(&source, "source-admin", "source-password").await;
    for scope in ["config", "data"] {
        export_backup(&source, &token, scope).await;
    }

    for (table, field, id) in [
        ("global_models", "supported_capabilities", "source-model"),
        ("provider_api_keys", "allowed_models", "channel-key"),
        ("provider_api_keys", "locked_models", "channel-key"),
        ("provider_api_keys", "model_include_patterns", "channel-key"),
        ("provider_api_keys", "model_exclude_patterns", "channel-key"),
        ("provider_api_keys", "api_formats", "channel-key"),
        (
            "provider_api_keys",
            "allow_auth_channel_mismatch_formats",
            "channel-key",
        ),
    ] {
        let original: Option<String> =
            sqlx::query_scalar(&format!("SELECT {field} FROM {table} WHERE id = ?"))
                .bind(id)
                .fetch_one(&source_pool)
                .await
                .unwrap();
        let update = format!("UPDATE {table} SET {field} = ? WHERE id = ?");
        for corrupted in [json!(["openai:chat", 42]), json!({"models": []})] {
            sqlx::query(&update)
                .bind(corrupted.to_string())
                .bind(id)
                .execute(&source_pool)
                .await
                .unwrap();
            for scope in ["config", "data"] {
                let (status, body) = backup_request(
                    &source,
                    http::Method::GET,
                    &format!("/api/admin/system/{scope}/export"),
                    Some(&token),
                    None,
                )
                .await;
                assert!(
                    !status.is_success(),
                    "{table}.{field} = {corrupted} must fail {scope} export: {body}"
                );
                for part in [
                    "exported_at",
                    "config_data",
                    "user_data",
                    "providers",
                    "global_models",
                    "proxy_nodes",
                    "system_configs",
                    "routing_strategy",
                    "users",
                    "standalone_keys",
                    "usage_aggregates",
                ] {
                    assert!(
                        body.get(part).is_none(),
                        "{table}.{field} leaked partial {part}: {body}"
                    );
                }
            }
        }
        sqlx::query(&update)
            .bind(original)
            .bind(id)
            .execute(&source_pool)
            .await
            .unwrap();
        for scope in ["config", "data"] {
            export_backup(&source, &token, scope).await;
        }
    }
    source_pool.close().await;
}

#[tokio::test]
async fn legacy_backup_without_admin_or_source_ids_restores_all_supplied_data() {
    use aether_crypto::{decrypt_python_fernet_ciphertext, encrypt_python_fernet_plaintext};

    let (source, source_pool, _source_file) =
        backup_gateway("source-admin", "source-password", "source-encryption").await;
    let (target, target_pool, _target_file) =
        backup_gateway("target-admin", "target-password", "target-encryption").await;
    seed_backup_source(&source_pool).await;
    // The single-user migration retired groups but retained their scalar setting.
    // A backup must preserve that setting even though its group no longer exists.
    sqlx::query("INSERT INTO system_configs (id, key, value, created_at, updated_at) VALUES ('legacy-group-setting', 'default_user_group_id', '\"retired-group\"', 1704067200, 1704067200) ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at")
        .execute(&source_pool).await.unwrap();
    sqlx::query("INSERT INTO user_preferences (id, user_id, bio, theme, created_at, updated_at) VALUES ('target-preferences', 'target-admin', 'keep this administrator', 'light', 1704067200, 1704067200)")
        .execute(&target_pool).await.unwrap();
    let source_token = backup_login(&source, "source-admin", "source-password").await;
    let target_token = backup_login(&target, "target-admin", "target-password").await;
    let backup = export_backup(&source, &source_token, "data").await;
    let admin_before = backup_table_snapshot(
        &target_pool,
        &["users", "user_preferences", "user_sessions"],
    )
    .await;
    let mut legacy = backup.clone();
    legacy["version"] = json!("1.0");
    legacy["config_data"]["version"] = json!("2.3");
    legacy["user_data"]["version"] = json!("1.5");
    legacy["merge_mode"] = json!("overwrite");
    for provider in legacy["config_data"]["providers"].as_array_mut().unwrap() {
        provider.as_object_mut().unwrap().remove("id");
        for key in provider["api_keys"].as_array_mut().unwrap() {
            key.as_object_mut().unwrap().remove("id");
        }
    }
    let mut legacy_key = backup["user_data"]["users"][0]["api_keys"][0].clone();
    legacy_key["is_standalone"] = json!(true);
    legacy_key["key_encrypted"] =
        json!(encrypt_python_fernet_plaintext("source-encryption", "sk-backup-user").unwrap());
    legacy_key["wallet"] = Value::Null;
    let key = legacy_key.as_object_mut().unwrap();
    key.remove("user_id");
    key.remove("expires_at_unix_secs");
    key.insert("expires_at".to_string(), json!("2100-01-01T00:00:00Z"));
    for field in ["last_used_at", "created_at", "updated_at"] {
        let value = key.remove(&format!("{field}_unix_secs")).unwrap();
        key.insert(field.to_string(), value);
    }
    legacy["user_data"]["users"] = json!([]);
    legacy["user_data"]["standalone_keys"] = json!([legacy_key]);
    legacy["user_data"]
        .as_object_mut()
        .unwrap()
        .remove("provider_names");

    let (status, response) = backup_request(
        &target,
        http::Method::POST,
        "/api/admin/system/data/import",
        Some(&target_token),
        Some(&legacy),
    )
    .await;
    assert_eq!(status, http::StatusCode::OK, "legacy import: {response}");
    assert_eq!(response["users"]["reauthentication_required"], false);
    assert_eq!(response["users"]["stats"]["users"]["updated"], 0);
    assert_eq!(response["users"]["stats"]["standalone_keys"]["created"], 1);
    assert_eq!(
        backup_table_snapshot(
            &target_pool,
            &["users", "user_preferences", "user_sessions"]
        )
        .await,
        admin_before,
        "omitted administrator data must preserve the current profile, password and sessions"
    );
    let restored = export_backup(&target, &target_token, "data").await;
    assert_eq!(
        restored["config_data"]["system_configs"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == "default_user_group_id")
            .unwrap()["value"],
        "retired-group"
    );
    assert_eq!(
        config_backup_content(&restored["config_data"]),
        config_backup_content(&backup["config_data"])
    );
    let user = &restored["user_data"]["users"][0];
    assert_eq!(user["username"], "target-admin");
    assert_eq!(user["preferences"]["bio"], "keep this administrator");
    assert_eq!(user["api_keys"], json!([]));
    assert_eq!(user["request_count"], 12);
    assert_eq!(user["total_tokens"], 150);
    let restored_keys = restored["user_data"]["standalone_keys"].as_array().unwrap();
    assert_eq!(restored_keys.len(), 1);
    let restored_key = &restored_keys[0];
    let mut expected_key = backup["user_data"]["users"][0]["api_keys"][0].clone();
    expected_key["api_key_id"] = restored_key["api_key_id"].clone();
    expected_key["user_id"] = json!("target-admin");
    expected_key["is_standalone"] = json!(true);
    expected_key["expires_at_unix_secs"] = json!(4_102_444_800u64);
    // Restoration records a target-local modification time.
    let modified_at = restored_key["updated_at_unix_secs"].as_u64().unwrap();
    assert!(modified_at >= expected_key["updated_at_unix_secs"].as_u64().unwrap());
    assert!(modified_at <= chrono::Utc::now().timestamp() as u64);
    expected_key["updated_at_unix_secs"] = json!(modified_at);
    assert_eq!(*restored_key, expected_key);
    let encrypted: String =
        sqlx::query_scalar("SELECT key_encrypted FROM api_keys WHERE is_standalone = 1")
            .fetch_one(&target_pool)
            .await
            .unwrap();
    assert_eq!(
        decrypt_python_fernet_ciphertext("target-encryption", &encrypted).unwrap(),
        "sk-backup-user"
    );
    assert!(decrypt_python_fernet_ciphertext("source-encryption", &encrypted).is_err());

    let aggregates = &restored["user_data"]["usage_aggregates"];
    assert_eq!(
        aggregates["stats_daily"],
        backup["user_data"]["usage_aggregates"]["stats_daily"]
    );
    let user_daily = aggregates["stats_user_daily"].as_array().unwrap();
    assert_eq!(user_daily.len(), 1);
    assert_eq!(user_daily[0]["user_id"], "target-admin");
    assert_eq!(user_daily[0]["total_requests"], 12);
    assert_eq!(user_daily[0]["input_tokens"], 90);
    assert_eq!(user_daily[0]["output_tokens"], 60);
    let mut expected_key_daily =
        backup["user_data"]["usage_aggregates"]["stats_daily_api_key"].clone();
    expected_key_daily[0]["api_key_id"] = restored_key["api_key_id"].clone();
    assert_eq!(aggregates["stats_daily_api_key"], expected_key_daily);
    source_pool.close().await;
    target_pool.close().await;
}
