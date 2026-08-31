use super::*;

fn production_source(source: &str) -> &str {
    source.split("#[cfg(test)]").next().unwrap_or(source)
}

#[test]
fn handlers_do_not_inline_sql_queries() {
    assert_no_sqlx_queries("src/handlers");
}

#[test]
fn gateway_runtime_does_not_inline_sql_queries() {
    assert_no_sqlx_queries("src/state/runtime");
}

#[test]
fn aether_data_backend_pool_modules_do_not_own_maintenance_sql() {
    for path in [
        "crates/aether-data/runtime/src/backend/postgres.rs",
        "crates/aether-data/runtime/src/backend/mysql.rs",
        "crates/aether-data/runtime/src/backend/sqlite.rs",
    ] {
        let source = read_workspace_file(path);
        let production = production_source(&source);
        for forbidden in [
            "run_table_maintenance(",
            "aggregate_wallet_daily_usage(",
            "aggregate_stats_hourly(",
            "aggregate_stats_daily(",
            "find_system_config_value(",
            "list_system_config_entries(",
            "upsert_system_config_entry(",
            "read_admin_system_stats(",
            "sqlx::query(",
            "sqlx::query_scalar",
            "sqlx::raw_sql(",
        ] {
            assert!(
                !production.contains(forbidden),
                "{path} should stay focused on pool and repository construction instead of owning maintenance SQL via {forbidden}"
            );
        }
    }

    let maintenance = read_workspace_file("crates/aether-data/runtime/src/backend/maintenance.rs");
    for pattern in [
        "Self::Postgres(postgres) => postgres.run_table_maintenance(table_names).await",
        "Self::Mysql(mysql) => mysql.run_table_maintenance(table_names).await",
        "Self::Sqlite(sqlite) => sqlite.run_table_maintenance(table_names).await",
        "Self::Postgres(postgres) => postgres.aggregate_wallet_daily_usage(input).await",
        "Self::Mysql(mysql) => mysql.aggregate_wallet_daily_usage(input).await",
        "Self::Sqlite(sqlite) => sqlite.aggregate_wallet_daily_usage(input).await",
        "Self::Postgres(postgres) => postgres.aggregate_stats_hourly(input).await",
        "Self::Mysql(mysql) => mysql.aggregate_stats_hourly(input).await",
        "Self::Sqlite(sqlite) => sqlite.aggregate_stats_hourly(input).await",
        "Self::Postgres(postgres) => postgres.aggregate_stats_daily(input).await",
        "Self::Mysql(mysql) => mysql.aggregate_stats_daily(input).await",
        "Self::Sqlite(sqlite) => sqlite.aggregate_stats_daily(input).await",
    ] {
        assert!(
            maintenance.contains(pattern),
            "backend/maintenance.rs should own SQL-driver maintenance dispatch {pattern}"
        );
    }
}
