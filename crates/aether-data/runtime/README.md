# aether-data

`aether-data` composes the SQLite data backend, in-memory repositories, and
migration, backfill, export, and maintenance workflows. Shared DTOs, repository
traits, and errors live in `../contracts` (`aether-data-contracts`). The SQLite
adapter in `../adapters/sqlite` owns request-path SQL and executable migrations.

## Directory Map

| Path | Responsibility |
|---|---|
| `src/database.rs` | Shared database configuration and historical driver metadata. |
| `src/config.rs` | Data-layer configuration validation. |
| `src/maintenance.rs` | Maintenance DTOs and aggregation summaries. |
| `src/driver/sqlite.rs` | Compatibility re-export of SQLite pools and adapter primitives. |
| `src/repository` | Contract and SQLite adapter re-exports, plus in-memory implementations. |
| `src/backend` | SQLite repository composition and optional-backend handles. |
| `src/backend/{maintenance,stats,wallet,system}` | Maintenance, usage aggregation, wallet ledger, and system configuration SQL. |
| `src/lifecycle/migrate` | SQLite migration and startup entry points. |
| `src/lifecycle/backfill` | SQLite backfill execution and discovery. |
| `src/lifecycle/export` | SQLite JSONL export, import, and database copy. |
| `../adapters/sqlite/migrations` | Published, versioned SQLx migrations embedded by the adapter. |
| `backfills/sqlite` | Versioned SQLite repairs recorded in `schema_backfills`. |
| `schema/logical` | Logical table definitions, including historical schema shapes. |
| `schema/drivers/sqlite` | Source fragments that reproduce the published SQLite baseline. |
| `schema/generated` | Checked-in SQL generator output for audit and drift checks. |
| `schema/drivers/{postgres,mysql}`, `schema/bootstrap/postgres`, `backfills/{postgres,mysql}` | Historical SQL retained for review; not embedded or executed by the runtime. |

## Runtime Boundary

SQLite is the only supported SQL backend. PostgreSQL and MySQL names remain in
shared metadata so historical inputs can be identified and rejected explicitly;
they do not provide connection or deployment capabilities. SQL deployment is
single-node. There is no PostgreSQL bootstrap snapshot build step.

The `sqlite` feature is enabled by default:

```bash
cargo check -p aether-data
cargo check -p aether-data --no-default-features
cargo check -p aether-data --no-default-features --features sqlite
cargo check -p aether-data --all-features
```

`--no-default-features` retains contracts, in-memory implementations, and the
empty backend configuration. Configuring a SQLite database in that build returns
an explicit disabled-driver error. The `postgres`, `mysql`, and `all-drivers`
features do not exist. The gateway uses the default SQLite feature.

## Layering Rules

1. Put cross-crate contracts in `aether-data-contracts`.
2. Put pools, transaction primitives, and request-path repository SQL in
   `aether-data-sqlite`.
3. Keep `src/repository/<domain>/mod.rs` as the compatibility import boundary;
   `memory.rs` owns an in-memory implementation when one exists.
4. Wire repositories through `src/backend/read.rs` and `src/backend/write.rs`.
   Keep backend maintenance SQL in the focused maintenance modules.
5. Keep migration, backfill, and export operations in `lifecycle`, outside normal
   request handling.

SQLite stores JSON as text, booleans and Unix timestamps as integers, and money
values using the existing real-valued schema contract. Repository interfaces use
Rust values such as `serde_json::Value`; callers should not depend on physical SQL
types. PostgreSQL/MySQL syntax in historical schema generator fixtures does not
expand the runtime driver matrix.

## Schema Maintenance

The executable migration sequence in `../adapters/sqlite/migrations` determines
the current database schema. Published SQL files keep their versions and
checksums: schema upgrades require a new migration, not a rewritten baseline.
Historical tables and columns in logical/generated SQL are audit fixtures and
must not be mistaken for tables present after all current migrations.

```bash
bash crates/aether-data/runtime/schema/compose_schema.sh check
bash crates/aether-data/runtime/schema/compose_schema.sh generate
```

`check` verifies generated SQL and logical coverage of the SQLite migration
sources, then compares the SQLite baseline with its manifest. It does not connect
to a database or write SQL. `generate` updates audit output for the historical
PostgreSQL, MySQL, and SQLite dialects; it does not change executable migrations.

`compose` and `split` reproduce the existing SQLite baseline and its source
fragments. They are maintenance tools for that published artifact, not the schema
upgrade path. Neither command targets the removed PostgreSQL/MySQL adapters.

When changing table structure, update the relevant logical definition and audit
output, add a new SQLite migration, update shared contracts and SQLite
repositories, and then wire any new repository through the backend. Preserve
existing migration history and explicit import compatibility behavior.

Existing migration and backfill regressions run with the normal SQLite feature:

```bash
cargo test -p aether-data --lib lifecycle::migrate::tests
cargo test -p aether-data --lib lifecycle::backfill::tests
```

See `schema/README.md` for the distinction between executable migrations and
historical maintenance inputs.
