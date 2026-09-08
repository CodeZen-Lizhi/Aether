# SQLite Runtime and Backup Contract

## 1. Scope / Trigger

Use this contract when changing data backend features, gateway database arguments,
schema maintenance, or JSONL export/import/copy. Executable SQLite migrations are
the schema authority; logical/generated SQL also contains historical shapes.

## 2. Signatures

- `export_database_jsonl(SqlDatabaseConfig, Vec<ExportDomain>, u64) -> Result<String, DataLayerError>`
- `import_database_jsonl(SqlDatabaseConfig, &str) -> Result<usize, DataLayerError>`
- `copy_database_records(SqlDatabaseConfig, SqlDatabaseConfig, Vec<ExportDomain>, u64, DataCopyOptions) -> Result<usize, DataLayerError>`
- Gateway commands are `aether-gateway export`, `import`, and `copy`.
- `bash crates/aether-data/runtime/schema/compose_schema.sh check` is read-only.

Implementations: `crates/aether-data/runtime/src/lifecycle/export.rs`, its
`export/sqlite.rs` child, and `apps/aether-gateway/src/main.rs`.

## 3. Contracts

- Runtime features: default `sqlite`; `--no-default-features` supports contracts,
  memory repositories and empty backends, but cannot open a database. No empty
  `postgres` or `mysql` feature may reactivate partial code.
- Historical `DatabaseDriver` enum values remain readable as export metadata.
  Connection capability is checked separately. Gateway accepts SQLite and
  `single-node`; legacy PostgreSQL URL arguments fail before connection.
- Empty export domains select all current business tables, excluding SQLite
  internals, `_sqlx_migrations`, and `schema_backfills`.
- Default domains exclude retired OAuth/user-group tables. Auxiliary tables
  include `payment_gateway_configs` (primary key `provider`) and
  `referral_rewards` (primary key `id`). Keep `auth_modules` historical data.
- JSONL versions 1 and 2 remain readable. Import uses primary-key UPSERT, not
  REPLACE, and one transaction. Foreign keys are deferred until commit, never
  disabled; commit failures roll back the entire import.
- `omit_request_body_details` removes inline usage bodies, body blob rows and
  HTTP body-reference/capture fields, retaining usage and routing snapshots.
- Preserve integer timestamp units, including the legacy seconds-valued
  `usage.created_at_unix_ms`, and byte arrays / PostgreSQL `\x` hex imports.

## 4. Validation & Error Matrix

| Input | Behavior |
|---|---|
| PostgreSQL/MySQL connection config | Explicit unsupported-driver error |
| Historical PostgreSQL/MySQL manifest metadata | Parse normally |
| Explicit retired export domain | `InvalidInput`, before querying missing tables |
| Retired domain in old manifest, no rows | Skip without querying its former table |
| Retired domain containing rows | Error and atomic rollback |
| Unknown auxiliary table or non-null unknown column | Error; never silently discard data |
| Unknown null historical column | Ignore as existing format compatibility |
| Malformed binary hex / invalid integer timestamp | Input error, never a panic |
| Unresolved foreign key at commit | Error and rollback; deferred mode resets |

## 5. Good / Base / Bad Cases

- Good: migrate a temporary SQLite database, export all current tables, then
  import/copy into another migrated database with all referenced records intact.
- Base: an empty migrated database exports successfully.
- Bad: use a historical generated table list to select a table dropped by a
  later migration, or omit current tables because their UI was removed.

## 6. Tests Required

Use the existing `runtime/src/lifecycle/export/tests.rs`: compare actual migrated
tables with coverage, execute the default export, exercise both copy modes,
assert rollback on late row and deferred FK failures, and retain child rows on
parent updates. Use actual migrated foreign keys, such as
`referral_rewards.inviter_user_id -> users.id` and
`background_task_events.run_id -> background_task_runs.id`; the baseline
`api_keys.user_id` and `models.provider_id` columns have no declared foreign key.
Existing migrate/backfill/backend tests run with `test + sqlite`.
Check supported feature combinations and run the schema script's `check` mode.
Do not rewrite published SQL or connect to a user's database for these checks.

## 7. Wrong vs Correct

Wrong: define `postgres = []` while leaving `cfg(feature = "postgres")` paths
that require absent drivers, or assume every auxiliary table uses an `id` key.

Correct: advertise only implemented features and keep table-specific keys in
the explicit auxiliary whitelist; derive completeness from the migrated schema.
