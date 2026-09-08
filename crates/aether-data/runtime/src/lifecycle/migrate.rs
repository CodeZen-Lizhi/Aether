//! Runtime database migration entry points.
//!
//! The SQLite adapter owns its migrator and startup preparation. The facade keeps
//! the established public entry points used by gateway bootstrap code.

#[cfg(feature = "sqlite")]
mod sqlite;
mod types;

#[cfg(all(test, feature = "sqlite"))]
mod tests;

pub use types::PendingMigrationInfo;

#[cfg(feature = "sqlite")]
use sqlx::migrate::MigrateError;

#[cfg(feature = "sqlite")]
pub async fn run_sqlite_migrations(pool: &sqlx::SqlitePool) -> Result<(), MigrateError> {
    sqlite::run_migrations(pool).await
}

#[cfg(feature = "sqlite")]
pub async fn pending_sqlite_migrations(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<PendingMigrationInfo>, MigrateError> {
    sqlite::pending_migrations(pool).await
}

#[cfg(feature = "sqlite")]
pub async fn prepare_sqlite_database_for_startup(
    pool: &sqlx::SqlitePool,
) -> Result<Vec<PendingMigrationInfo>, MigrateError> {
    sqlite::prepare_database_for_startup(pool).await
}
