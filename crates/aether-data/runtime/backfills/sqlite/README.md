SQLite lifecycle backfills live here and are embedded by `aether-data` when the
`sqlite` feature is enabled. Applied versions and checksums are recorded in
`schema_backfills`. Versions inherited from historical PostgreSQL repairs stay
unchanged; the current runtime does not execute those old driver scripts.
