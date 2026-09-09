# Administrator JSON Backup Contract

## 1. Scope / Trigger

Use this contract when changing the system-settings configuration or complete
backup UI, `/api/admin/system/` backup routes, or their database access. These
administrator JSON documents are distinct from the CLI JSONL database format
in [SQLite lifecycle](../../aether-data/backend/sqlite-lifecycle.md).

## 2. Signatures

- `GET /api/admin/system/{config,data}/export` builds a complete JSON
  response before returning success.
- `POST /api/admin/system/{config,data}/import` accepts a document and
  `merge_mode: skip | overwrite | error` under authenticated administrator access.
- Internal user-only import/export helpers use the same transaction protocol;
  the UI restores user content through the complete backup routes.
- `SqliteBackend::begin_admin_backup(write: bool) -> AdminBackupSession` owns
  the operation's transaction. `AdminBackupState` exposes only repositories and
  connection helpers bound to that session, plus explicit cryptographic helpers.
- `SqliteBackupTransaction::{source,commit,rollback}` manages a
  `SqliteConnectionSource`. Pool-backed sources retain normal repository behavior;
  transaction-backed sources borrow the owner's single connection.

## 3. Contracts

- New documents do not contain a format `version`. Historical `version`
  metadata of any JSON type does not determine compatibility. Validate actual
  content and relationships. Routing revisions and proxy `config_version` remain
  domain data.
- Configuration contains `exported_at`, `global_models`, `providers`,
  `proxy_nodes`, `system_configs`, and optional `routing_strategy`. Complete
  backup contains `config_data` and `user_data` from the same database snapshot.
- User data contains administrator `users`, `standalone_keys`, `provider_names`,
  and `usage_aggregates`. The API Key record includes `is_locked` (default false
  for old documents); export both ordinary and standalone keys, including inactive
  keys and keys for which only a hash is stored.
- Known historical shapes are explicitly normalized: missing provider/key source
  IDs, `supported_endpoints`, old Key timestamp names, missing provider-name maps
  in complete documents, and empty administrator arrays. An absent administrator
  preserves the target's profile, password, preferences and sessions; any unique
  source owner in Key/statistics data maps to the authenticated target owner.
- Keep retired scalar configuration such as `default_user_group_id` unchanged.
  Nonempty retired domains (for example user groups or wallets) cannot disappear
  silently: reject them if the personal application cannot restore them.
- Source duplicates that normalize to one model, provider, endpoint, model mapping
  or config key are errors. Explicit target duplicate handling follows the selected
  merge mode; skipped duplicates are not failed rows.
- All configuration, Keys, profile/password/preferences, session revocations,
  routing history, and usage writes use one write transaction (`BEGIN IMMEDIATE`,
  deferred foreign keys). Repository-local transactions become SQLx savepoints on
  the shared connection. Release the connection guard before another repository
  call. No ordinary pool reads/writes or cache mutation inside the backup scope.
- Any validation, SQL, credential or commit failure aborts the operation; do not
  collect errors and continue. Dropping the scope before finalization rolls back.
  Once finalization starts, a detached task owns commit, application cache
  invalidation, external-model cache cleanup and mutation-lease release together.
  A cancelled HTTP request must not strand committed data behind stale caches.
  Lease guards also release on cancellation before finalization.
- Export uses one read transaction. Decrypt all required credentials and validate
  all components before returning JSON. Never truncate malformed string lists,
  unknown nonempty fields, or unsupported API formats. Re-encrypt imported secrets
  using the target's encryption key; never return secret values in diagnostics.
- Preserve all supplied statistics rows (including zero-valued rows), their
  recorded names, costs, and `is_complete`; do not manufacture completion.
- The UI creates downloads only after a complete successful response. Import
  failures retain the preview for retry and never display a completion result.

## 4. Validation & Error Matrix

| Case | Result |
|---|---|
| Missing, old, or arbitrary format version | Determine compatibility from content |
| Valid older file with omitted administrator and source IDs | Restore supplied config/Keys/statistics, preserve target admin |
| Invalid shape, ambiguous ownership, unresolved reference, unsupported nonempty field | HTTP 400, no writes committed |
| Missing usable SQLite backend | HTTP 503, no writes |
| Late SQL error, failed preference write or deferred FK at commit | Failed response, entire transaction rolled back |
| Unreadable export credentials or malformed persisted list | Failed response without backup fragments |
| Cancellation while importing, before finalization | Rollback and release mutation lease |
| Cancellation after finalization is dispatched | Finish commit or rollback; publish cache invalidation only after commit |

## 5. Good / Base / Bad Cases

- Good: export from one migrated database, import to another with a different
  encryption key, and export again with all Key state, configuration and statistics
  retained after intentional target-ID remapping.
- Base: a valid personal administrator with empty configuration and Key arrays can
  be backed up; no version marker is required.
- Bad: wrap independent pool writes with a transaction that they never use, swallow
  an item error, filter independent Keys from the backup, or clear caches only in
  the HTTP future after awaiting COMMIT.

## 6. Tests Required

- `apps/aether-gateway/tests/admin_unsigned_identity_headers.rs`: authenticated
  cross-instance roundtrips; old content shapes; skip/overwrite/error; locked,
  inactive, standalone and hash-only Keys; late config/preferences failures with
  full table comparisons; password/session survival on failure and revocation on
  success; corrupt credentials/lists; zero statistics and completion flags.
- `request/system/backup_state/tests.rs`: detach finalization before it runs,
  cancel an uncommitted scope, and fail a deferred FK at commit. Assert independent
  database reads, already-warmed caches, external-model KV and lease release.
- `crates/aether-data/adapters/sqlite/tests/backup_transaction.rs`: shared
  repository transaction/savepoints, late failure rollback, owner/guard lifetimes,
  cancellation, commit failure, and snapshot consistency with a concurrent writer.
- `aether-admin::system`, `request/system/user_backup::tests`, and the affected
  frontend import/export tests validate compatibility, error handling and downloads.
  Use only synthetic credentials in committed fixtures.

## 7. Wrong vs Correct

Wrong: `pool.begin()` followed by normal `AppState` repository calls and a loop
that appends row errors to a success payload.

Correct: create one `AdminBackupSession`, construct every participating repository
from its `SqliteConnectionSource`, propagate any failure, and finalize once after
all writes are complete. Keep finalization alive across HTTP cancellation.
