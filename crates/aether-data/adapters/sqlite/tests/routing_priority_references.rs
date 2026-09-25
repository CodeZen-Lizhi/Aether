use aether_data_sqlite::{run_migrations, SqlitePool};
use serde_json::{json, Value};
use sqlx::sqlite::SqlitePoolOptions;

/// 本次新增迁移用于验证升级前孤立数据的修复。
const REPAIR: &str =
    include_str!("../migrations/20260925000000_repair_routing_priority_references.sql");

/// 创建真实迁移后的隔离数据库，并保存两个供应商及渠道密钥。
async fn database() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    run_migrations(&pool).await.unwrap();
    sqlx::raw_sql("INSERT INTO providers (id,name,provider_type,created_at,updated_at) VALUES ('keep','Keep','custom',1,1),('remove','Remove','custom',1,1);
        INSERT INTO provider_api_keys (id,provider_id,name,api_key,created_at,updated_at) VALUES ('keep-key','keep','Keep','synthetic',1,1),('remove-key','remove','Remove','synthetic',1,1);")
        .execute(&pool).await.unwrap();
    pool
}

/// 构造混合优先级、限制、条件和停止标记，确保修复不会重建或扩大策略。
fn config() -> Value {
    json!({"custom_metadata":{"preserve":true},"rules":[{
        "id":"ordering","enabled":false,"conditions":{"model":"unchanged"},"stop_processing":true,
        "actions":[
            {"type":"set_provider_priority","provider_id":"remove","priority":1},
            {"type":"set_provider_priority","provider_id":"keep","priority":2},
            {"type":"set_provider_priority","provider_id":"remove","priority":3},
            {"type":"set_key_priority","key_id":"remove-key","priority":4},
            {"type":"restrict_providers","provider_ids":["remove"]},
            {"type":"restrict_keys","key_ids":["remove-key"]}
        ]}],"model_policies":[{"model":"sample","allowed_providers":["remove"],"allowed_keys":["remove-key"],
            "provider_priority_overrides":{"remove":1,"keep":2},"key_priority_overrides":{"remove-key":3,"keep-key":4}}]})
}

/// 写入真实当前配置并保留一份原始历史版本。
async fn seed(pool: &SqlitePool, value: &Value) {
    sqlx::query("INSERT INTO routing_groups (id,name,config_json,version,created_at,updated_at) VALUES ('g','Group',?,1,1,1)")
        .bind(value.to_string()).execute(pool).await.unwrap();
    sqlx::query("INSERT INTO routing_group_versions (id,group_id,version,config_json,created_at) VALUES ('history','g',1,?,1)")
        .bind(value.to_string()).execute(pool).await.unwrap();
}

/// 从独立查询读回持久化内容，避免只断言调用返回值。
async fn stored(pool: &SqlitePool) -> Value {
    let text: String = sqlx::query_scalar("SELECT config_json FROM routing_groups WHERE id='g'")
        .fetch_one(pool)
        .await
        .unwrap();
    serde_json::from_str(&text).unwrap()
}

/// 只移除已删除实体的无效优先级，其余数据逐字段保持一致。
fn expected() -> Value {
    let mut value = config();
    value["rules"][0]["actions"] = json!([
        {"type":"set_provider_priority","provider_id":"keep","priority":2},
        {"type":"restrict_providers","provider_ids":["remove"]},
        {"type":"restrict_keys","key_ids":["remove-key"]}
    ]);
    value["model_policies"][0]["provider_priority_overrides"] = json!({"keep":2});
    value["model_policies"][0]["key_priority_overrides"] = json!({"keep-key":4});
    value
}

/// 删除与清理一起回滚；提交后旧配置再写入也不会复活孤立优先级。
#[tokio::test]
async fn deletion_is_atomic_and_stale_save_cannot_restore_priorities() {
    let pool = database().await;
    let original = config();
    seed(&pool, &original).await;
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("DELETE FROM provider_api_keys WHERE id='remove-key'")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("DELETE FROM providers WHERE id='remove'")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(stored(&pool).await, original);
    sqlx::query("DELETE FROM provider_api_keys WHERE id='remove-key'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM providers WHERE id='remove'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(stored(&pool).await, expected());
    sqlx::query("UPDATE routing_groups SET config_json=? WHERE id='g'")
        .bind(original.to_string())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(stored(&pool).await, expected());
    let history: String =
        sqlx::query_scalar("SELECT config_json FROM routing_group_versions WHERE id='history'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(serde_json::from_str::<Value>(&history).unwrap(), original);
    let keys: i64 =
        sqlx::query_scalar("SELECT count(*) FROM provider_api_keys WHERE id='keep-key'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(keys, 1);
}

/// 升级前已有孤立记录能修复，空规则的停止语义保留，且重复合法保存不改变结果。
#[tokio::test]
async fn upgrade_repairs_existing_orphans_without_erasing_rules() {
    let pool = database().await;
    sqlx::raw_sql(
        "DROP TRIGGER routing_priorities_after_provider_delete;
        DROP TRIGGER routing_priorities_after_key_delete;
        DROP TRIGGER routing_priorities_after_insert;
        DROP TRIGGER routing_priorities_after_update;
        DROP VIEW routing_priority_repairs;",
    )
    .execute(&pool)
    .await
    .unwrap();
    seed(&pool, &config()).await;
    sqlx::query("DELETE FROM provider_api_keys WHERE id='remove-key'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM providers WHERE id='remove'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(stored(&pool).await, config());
    sqlx::raw_sql(REPAIR).execute(&pool).await.unwrap();
    assert_eq!(stored(&pool).await, expected());
    let only_stale = json!({"rules":[{"id":"stop","stop_processing":true,"actions":[{"type":"set_provider_priority","provider_id":"missing","priority":1}]}]});
    sqlx::query("UPDATE routing_groups SET config_json=? WHERE id='g'")
        .bind(only_stale.to_string())
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        stored(&pool).await,
        json!({"rules":[{"id":"stop","stop_processing":true,"actions":[]}]})
    );
}
