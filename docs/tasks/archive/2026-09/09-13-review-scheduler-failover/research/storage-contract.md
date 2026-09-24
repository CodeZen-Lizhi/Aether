# Atomic Health Settlement Contract

Implementation and targeted repository verification are complete in aether-data-contracts, aether-data memory, SQLite and both gateway catalog adapters. Gateway execution integration is owned by health/main and was not built by storage.

`ProviderCatalogWriteRepository::settle_key_health_attempt(&ProviderCatalogKeyHealthSettlement) -> Result<ProviderCatalogKeyHealthSettlementResult, DataLayerError>`

Gateway wrapper: `AppState::settle_provider_catalog_key_health_attempt(&ProviderCatalogKeyHealthSettlement)` with the same result enum and GatewayError.

The wrapper contains `attempt_id: String`, canonical `api_format: String`, `policy_version: u32`, original `attempt_started_at_unix_secs: u64`, `expected_credential: ProviderCatalogKeyOAuthCredentialFence`, exact nullable `expected_encrypted_auth_config: Option<String>`, `expected_circuit_epoch: u64`, and the existing `update: ProviderCatalogKeyHealthStateUpdate`. Capture credentials and circuit epoch BEFORE the upstream attempt. Epoch is `circuit_breaker_by_format[api_format].circuit_epoch`, absent means zero. Stored `health_policy_version` may be absent for migration; otherwise it must match the supplied version.

- Applied: receipt and health/circuit replacement committed together.
- Duplicate: the identity `(attempt_id, key_id, api_format, policy_version)` already committed; never score again, even if the supplied projection differs.
- Conflict: no receipt committed; reload and reproject the same terminal fact against current health/circuit JSON. Keep the original attempt ID, timestamp, credentials and epoch frozen.
- StaleGeneration: original credential, policy version or circuit epoch no longer matches. Do not rebase those fences to current values.
- MissingKey: no target remains; no update or receipt committed.
- Expired: attempt start is at least 24 hours old; diagnostic only. Do not manufacture a new timestamp or attempt ID.
- Err: transaction did not report success. Retain the original settlement for bounded retry; this must never cause another upstream model call.

Receipt retention is 24 hours from immutable attempt start, independently of request-candidate cleanup. Receipts remain beyond expiry until bounded opportunistic cleanup; all expired reports are rejected before insertion, preventing scoring after cleanup. Receipt rows contain identifiers and timestamps only, never ciphertext, model input, output or error bodies. SQLite uses a unique receipt identity and one database transaction; memory uses the same write lock for receipts and keys. Ordinary health CAS remains source compatible.

Credential ciphertext equality fences static keys and OAuth replacements; credential ABA (replacing a credential with byte-identical original ciphertext) requires the lifecycle owner to advance circuit_epoch. A trusted recharge/recovery signal must also advance circuit_epoch before admitting new probes. This API does not itself perform credential rotation or acquire/renew probe leases.

The gateway data adapter has a thin forwarding method in `apps/aether-gateway/src/data/state/catalog.rs` because the state wrapper delegates through GatewayDataState. Cache invalidation occurs after every repository result, including errors; missing writers are an error, never an apparent successful settlement.

## Integration Boundaries

- The API protects persisted state. Callers must not fall back to ordinary health CAS for a real chat attempt with a missing identity; that fallback has no receipt and is not cross-instance idempotent.
- A transient storage error needs a bounded retry/retained terminal fact on the caller side. Logging and dropping the fact alone does not implement the design's settlement retry requirement.
- The terminal candidate record remains independently owned. The minimal receipt table was necessary because candidate finalization has no shared health transaction API and its cleanup lifetime is unrelated to settlement replay.
- Migration `20260913000000_add_provider_health_settlements.sql` adds the table. Logical schema/audit SQL and the JSONL auxiliary-table export list include it. Administrator JSON config backups are a different format and do not export this operational receipt table; they are not the supported rollback artifact for replay-safe state restoration.
- No production database migration, upstream request, probe lease lifecycle or gateway build has been executed by storage.
- The requested channel send was attempted but the sandbox denied writing the channel lock under `/Users/zhenglizhi/.trellis/channels/` (EPERM). This file is the available integration handoff.

## Verification Evidence

- `cargo test -p aether-data -p aether-data-sqlite --lib health_settlement`: final run passed 4 tests. SQLite covers two independent file-backed connections, duplicate races, distinct-attempt CAS conflict/reprojection, restart replay, atomic rollback on injected SQL failure, credential/epoch fences and expired reports after receipt removal. Memory covers duplicate/CAS retry, credential/auth/epoch fences, expiry and subsequent authoritative reads. JSONL covers receipt export, import and repeated import.
- `cargo test -p aether-data --lib lifecycle::migrate::tests`: passed 12 tests after adding the new migration to the expected version list, including generated logical schema column equality.
- Existing compiled test `lifecycle::export::tests::sqlite_core_export_covers_every_portable_table`: passed, covering the new table in the default export inventory.
- `target/debug/aether-schema check`: passed. Audit SQL was generated with `cargo run -q -p aether-data-schema --bin aether-schema -- generate`; historical baseline migration was not modified.
- `cargo clippy -p aether-data-contracts -p aether-data-sqlite -p aether-data --lib --no-deps -- -D warnings -A clippy::default_constructed_unit_structs`: passed. The unsuppressed run reported only two existing unit-struct default calls at runtime/src/backend/mod.rs:103 and :108; those unrelated lines were not changed. This check also type-checked all changed repository packages.
- Changed Rust files were formatted directly and `git diff --check` passed. No gateway cargo command, full-workspace test, production migration or commit was run.

## Changed Files

- contracts/src/repository/provider_catalog/{settlement.rs,mod.rs,types.rs}: wrapper, result enum, shared fence semantics and backwards-compatible trait method.
- runtime/src/repository/provider_catalog/memory.rs and adapters/sqlite/src/provider_catalog.rs: atomic implementations and targeted regression tests.
- adapters/sqlite/migrations/20260913000000_add_provider_health_settlements.sql: additive receipt table and expiry index, deliberately no cascading key foreign key so deleting a key cannot erase recent deduplication evidence.
- runtime/schema/logical/002_provider_catalog.toml and its three generated audit SQL files: schema synchronization.
- runtime/src/lifecycle/{export.rs,export/tests.rs,migrate/tests.rs}: backup inventory/roundtrip and migration-version regression.
- apps/aether-gateway/src/{data/state/catalog.rs,state/catalog.rs}: thin delegation and cache invalidation.
