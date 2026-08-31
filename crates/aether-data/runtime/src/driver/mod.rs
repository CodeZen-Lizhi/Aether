//! Compatibility paths for database adapter crates.
//!
//! New adapter code belongs in `aether-data-postgres`, `aether-data-mysql`, or
//! `aether-data-sqlite`. These modules preserve existing `aether_data::driver`
//! imports while application-facing composition remains in `backend`.

#[cfg(feature = "sqlite")]
pub mod sqlite;
