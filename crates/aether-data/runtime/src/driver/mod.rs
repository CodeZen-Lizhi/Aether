//! Compatibility path for the SQLite adapter crate.
//!
//! SQLite adapter code belongs in `aether-data-sqlite`. This module preserves
//! existing `aether_data::driver`
//! imports while application-facing composition remains in `backend`.

#[cfg(feature = "sqlite")]
pub mod sqlite;
