pub mod memory;
#[cfg(feature = "sqlite")]
pub mod sqlite {
    pub use aether_data_sqlite::SqliteGeminiFileMappingRepository;
}
pub mod types {
    pub use aether_data_contracts::repository::gemini_file_mappings::*;
}

pub use aether_data_contracts::repository::gemini_file_mappings::{
    GeminiFileMappingListQuery, GeminiFileMappingMimeTypeCount, GeminiFileMappingReadRepository,
    GeminiFileMappingRepository, GeminiFileMappingStats, GeminiFileMappingWriteRepository,
    StoredGeminiFileMapping, StoredGeminiFileMappingListPage, UpsertGeminiFileMappingRecord,
};
#[cfg(feature = "sqlite")]
pub use aether_data_sqlite::SqliteGeminiFileMappingRepository;
pub use memory::InMemoryGeminiFileMappingRepository;
