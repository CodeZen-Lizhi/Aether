mod memory;

pub use aether_data_contracts::repository::auth_modules::{
    AuthModuleReadRepository, AuthModuleWriteRepository, StoredLdapModuleConfig,
    StoredOAuthProviderModuleConfig,
};
#[cfg(feature = "sqlite")]
pub use aether_data_sqlite::{SqliteAuthModuleReadRepository, SqliteAuthModuleRepository};
pub use memory::InMemoryAuthModuleReadRepository;
