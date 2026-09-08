# Aether Schema Source

This directory contains schema maintenance inputs and audit output. The runtime
uses only the SQLite migrations in `../../adapters/sqlite/migrations`.
PostgreSQL/MySQL dialect output and old bootstrap fragments remain useful for
historical inspection, but they do not supply runtime adapters or build inputs.

## Commands

```bash
bash crates/aether-data/runtime/schema/compose_schema.sh check
bash crates/aether-data/runtime/schema/compose_schema.sh generate
```

- `check` validates checked-in generator output, checks logical coverage of the
  current SQLite migration sources, and compares the published SQLite baseline
  with its fragment manifest. It is read-only and does not require a database.
- `generate` renders `logical/*.toml` into `generated/{postgres,mysql,sqlite}`.
  These files are audit artifacts, not executable runtime migrations.
- `compose` reproduces the published SQLite baseline from its existing manifest.
- `split` reproduces the SQLite baseline fragments from that published SQL.

`compose` and `split` retain their artifact-maintenance role. Do not use them to
rewrite a deployed migration: add a new migration for each schema change.
PostgreSQL/MySQL adapter paths are not command targets. There is no automatic
bootstrap snapshot generation during `aether-data` builds.

## Source and Artifact Ownership

| Path | Role |
|---|---|
| `logical/*.toml` | Human-maintained logical definitions, including historical table and column shapes. |
| `generated/{postgres,mysql,sqlite}/` | Machine-written dialect output for audit and drift detection. |
| `drivers/sqlite/baseline/manifest.txt` | Fragment order that reproduces the published SQLite baseline. |
| `../../adapters/sqlite/migrations/` | Authoritative versioned migration sequence executed by the runtime. |
| `drivers/{postgres,mysql}/`, `bootstrap/postgres/` | Historical SQL fragments; retained for inspection, not runtime startup. |
| `overrides/` | Reserved for documented dialect exceptions. |

Logical/generated SQL is not a snapshot of the current SQLite database. Later
migrations can deliberately retire historical tables or columns. Runtime
migration tests verify the current table and column set, account explicitly for
those retirements, and keep the SQLite migration/backfill history executable.

Generated files must remain synchronized with the generator and logical sources.
The generator can also be called directly:

```bash
cargo run -p aether-data-schema --bin aether-schema -- check
cargo run -p aether-data-schema --bin aether-schema -- generate
cargo run -p aether-data-schema --bin aether-schema -- print --driver sqlite
```

`print --driver postgres` and `print --driver mysql` remain static SQL rendering
operations. They do not enable those databases in `aether-data`.

## Baseline Check

The only executable baseline target is:

| Target | Executable SQL | Source manifest |
|---|---|---|
| SQLite baseline | `../../adapters/sqlite/migrations/20260403000000_baseline.sql` | `drivers/sqlite/baseline/manifest.txt` |

The existing migration regression
`cargo test -p aether-data split_baseline_sources_match_executable_migrations`
compares that manifest with the published baseline too. Leave historical
migration checksums unchanged when adjusting tooling or documentation.
