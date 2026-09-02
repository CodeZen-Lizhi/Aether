use std::sync::RwLock;

use aether_data_contracts::repository::candidate_selection::provider_model_mapping_api_format_covers;
use async_trait::async_trait;

use super::{
    MinimalCandidateSelectionReadRepository, StoredApiFormatCandidateRowsQuery,
    StoredMinimalCandidateSelectionRow, StoredRequestedModelCandidateRowsQuery,
};
use crate::DataLayerError;

#[derive(Debug, Default)]
pub struct InMemoryMinimalCandidateSelectionReadRepository {
    rows: RwLock<Vec<StoredMinimalCandidateSelectionRow>>,
}

impl InMemoryMinimalCandidateSelectionReadRepository {
    pub fn seed<I>(rows: I) -> Self
    where
        I: IntoIterator<Item = StoredMinimalCandidateSelectionRow>,
    {
        Self {
            rows: RwLock::new(rows.into_iter().collect()),
        }
    }
}

#[async_trait]
impl MinimalCandidateSelectionReadRepository for InMemoryMinimalCandidateSelectionReadRepository {
    async fn list_for_exact_api_format(
        &self,
        api_format: &str,
    ) -> Result<Vec<StoredMinimalCandidateSelectionRow>, DataLayerError> {
        let api_format = api_format.trim();
        let mut rows = self
            .rows
            .read()
            .expect("candidate selection repository lock")
            .iter()
            .filter(|row| {
                row.provider_is_active
                    && row.endpoint_is_active
                    && row.key_is_active
                    && row.model_is_active
                    && row.model_is_available
                    && api_format_matches(&row.endpoint_api_format, api_format)
                    && row.key_supports_api_format(api_format)
                    && key_auth_channel_matches(row, api_format)
            })
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.provider_id
                .cmp(&right.provider_id)
                .then(left.endpoint_id.cmp(&right.endpoint_id))
                .then(left.key_id.cmp(&right.key_id))
                .then(left.model_id.cmp(&right.model_id))
        });
        Ok(rows)
    }

    async fn list_for_exact_api_format_page(
        &self,
        query: &StoredApiFormatCandidateRowsQuery,
    ) -> Result<Vec<StoredMinimalCandidateSelectionRow>, DataLayerError> {
        Ok(self
            .list_for_exact_api_format(&query.api_format)
            .await?
            .into_iter()
            .skip(query.offset as usize)
            .take(query.limit as usize)
            .collect())
    }

    async fn list_for_exact_api_format_and_global_model(
        &self,
        api_format: &str,
        global_model_name: &str,
    ) -> Result<Vec<StoredMinimalCandidateSelectionRow>, DataLayerError> {
        let rows = self.list_for_exact_api_format(api_format).await?;
        Ok(rows
            .into_iter()
            .filter(|row| row.global_model_name == global_model_name)
            .collect())
    }

    async fn list_for_exact_api_format_and_requested_model(
        &self,
        api_format: &str,
        requested_model_name: &str,
    ) -> Result<Vec<StoredMinimalCandidateSelectionRow>, DataLayerError> {
        self.list_for_exact_api_format_and_requested_model_page(
            &StoredRequestedModelCandidateRowsQuery {
                api_format: api_format.to_string(),
                requested_model_name: requested_model_name.to_string(),
                offset: 0,
                limit: u32::MAX,
            },
        )
        .await
    }

    async fn list_for_exact_api_format_and_requested_model_page(
        &self,
        query: &StoredRequestedModelCandidateRowsQuery,
    ) -> Result<Vec<StoredMinimalCandidateSelectionRow>, DataLayerError> {
        let rows = self.list_for_exact_api_format(&query.api_format).await?;
        let mut rows = rows
            .into_iter()
            .filter(|row| {
                row_matches_requested_model(row, &query.requested_model_name, &query.api_format)
            })
            .collect::<Vec<_>>();
        rows.sort_by(|left, right| {
            left.global_model_name
                .cmp(&right.global_model_name)
                .then(left.provider_id.cmp(&right.provider_id))
                .then(left.endpoint_id.cmp(&right.endpoint_id))
                .then(left.key_id.cmp(&right.key_id))
                .then(left.model_id.cmp(&right.model_id))
        });
        Ok(rows
            .into_iter()
            .skip(query.offset as usize)
            .take(query.limit as usize)
            .collect())
    }
}

fn api_format_matches(left: &str, right: &str) -> bool {
    aether_ai_formats::api_format_alias_matches(left, right)
}

fn row_matches_requested_model(
    row: &StoredMinimalCandidateSelectionRow,
    requested_model_name: &str,
    api_format: &str,
) -> bool {
    (row_has_available_provider_model(row, api_format)
        && row.global_model_name == requested_model_name)
        || (row_default_provider_model_name_available(row, api_format)
            && row.model_provider_model_name == requested_model_name)
        || row
            .model_provider_model_mappings
            .as_ref()
            .is_some_and(|mappings| {
                mappings.iter().any(|mapping| {
                    mapping.api_formats.as_ref().is_none_or(|formats| {
                        formats.iter().any(|value| {
                            provider_model_mapping_api_format_covers(
                                &row.provider_type,
                                value,
                                api_format,
                            )
                        })
                    }) && mapping.endpoint_ids.as_ref().is_none_or(|endpoint_ids| {
                        endpoint_ids
                            .iter()
                            .any(|endpoint_id| endpoint_id == &row.endpoint_id)
                    }) && mapping.name == requested_model_name
                })
            })
}

fn row_has_available_provider_model(
    row: &StoredMinimalCandidateSelectionRow,
    api_format: &str,
) -> bool {
    row_mapping_matches_scope(row, api_format)
        || row_default_provider_model_name_available(row, api_format)
}

fn row_default_provider_model_name_available(
    row: &StoredMinimalCandidateSelectionRow,
    api_format: &str,
) -> bool {
    let Some(mappings) = row.model_provider_model_mappings.as_ref() else {
        return true;
    };
    let mut has_explicit_default_mapping = false;
    for mapping in mappings {
        if mapping.name != row.model_provider_model_name {
            continue;
        }
        has_explicit_default_mapping = true;
        if mapping_scope_matches(mapping, row, api_format) {
            return true;
        }
    }
    !has_explicit_default_mapping
}

fn row_mapping_matches_scope(row: &StoredMinimalCandidateSelectionRow, api_format: &str) -> bool {
    row.model_provider_model_mappings
        .as_ref()
        .is_some_and(|mappings| {
            mappings
                .iter()
                .any(|mapping| mapping_scope_matches(mapping, row, api_format))
        })
}

fn mapping_scope_matches(
    mapping: &super::StoredProviderModelMapping,
    row: &StoredMinimalCandidateSelectionRow,
    api_format: &str,
) -> bool {
    mapping.api_formats.as_ref().is_none_or(|formats| {
        formats.iter().any(|value| {
            provider_model_mapping_api_format_covers(&row.provider_type, value, api_format)
        })
    }) && mapping.endpoint_ids.as_ref().is_none_or(|endpoint_ids| {
        endpoint_ids
            .iter()
            .any(|endpoint_id| endpoint_id == &row.endpoint_id)
    })
}

fn key_auth_channel_matches(row: &StoredMinimalCandidateSelectionRow, _api_format: &str) -> bool {
    // Only non-OAuth key credentials participate in candidate selection now
    // that the provider catalog has no OAuth-backed provider types.
    !row.key_auth_type.trim().eq_ignore_ascii_case("oauth")
}

#[cfg(test)]
mod tests {
    use super::InMemoryMinimalCandidateSelectionReadRepository;
    use crate::repository::candidate_selection::{
        MinimalCandidateSelectionReadRepository, StoredApiFormatCandidateRowsQuery,
        StoredMinimalCandidateSelectionRow, StoredProviderModelMapping,
        StoredRequestedModelCandidateRowsQuery,
    };

    fn sample_row(
        provider_id: &str,
        api_format: &str,
        global_model_name: &str,
    ) -> StoredMinimalCandidateSelectionRow {
        StoredMinimalCandidateSelectionRow {
            provider_id: provider_id.to_string(),
            provider_name: provider_id.to_string(),
            provider_type: "custom".to_string(),
            provider_is_active: true,
            endpoint_id: format!("endpoint-{provider_id}"),
            endpoint_api_format: api_format.to_string(),
            endpoint_api_family: Some("openai".to_string()),
            endpoint_kind: Some("chat".to_string()),
            endpoint_is_active: true,
            key_id: format!("key-{provider_id}"),
            key_name: "prod".to_string(),
            key_auth_type: "api_key".to_string(),
            key_is_active: true,
            key_api_formats: Some(vec![api_format.to_string()]),
            key_allowed_models: None,
            key_capabilities: None,
            model_id: format!("model-{provider_id}"),
            global_model_id: "global-model-1".to_string(),
            global_model_name: global_model_name.to_string(),
            global_model_supports_streaming: Some(true),
            model_provider_model_name: global_model_name.to_string(),
            model_provider_model_mappings: None,
            model_supports_streaming: None,
            model_is_active: true,
            model_is_available: true,
        }
    }

    #[tokio::test]
    async fn filters_by_exact_api_format_and_global_model() {
        let repository = InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_row("provider-2", "openai:chat", "gpt-4.1"),
            sample_row("provider-1", "openai:chat", "gpt-4.1"),
            sample_row("provider-3", "openai:responses", "gpt-4.1"),
            sample_row("provider-4", "openai:chat", "gpt-4.1-mini"),
        ]);

        let rows = repository
            .list_for_exact_api_format_and_global_model("openai:chat", "gpt-4.1")
            .await
            .expect("list should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].provider_id, "provider-1");
        assert_eq!(rows[1].provider_id, "provider-2");
    }

    #[tokio::test]
    async fn filters_by_exact_api_format_and_requested_model_aliases() {
        let mut mapped = sample_row("provider-1", "openai:chat", "gpt-4.1");
        mapped.model_provider_model_name = "provider-gpt-4.1".to_string();
        mapped.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
            name: "alias-gpt-4.1".to_string(),
            priority: 0,
            api_formats: Some(vec!["openai:chat".to_string()]),
            endpoint_ids: None,
            operations: None,
        }]);
        let repository = InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            mapped,
            sample_row("provider-2", "openai:chat", "gpt-4.1-mini"),
        ]);

        let rows = repository
            .list_for_exact_api_format_and_requested_model("openai:chat", "alias-gpt-4.1")
            .await
            .expect("list should succeed");

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_id, "provider-1");
    }

    #[tokio::test]
    async fn requested_model_filter_respects_endpoint_scoped_default_mapping() {
        let mut selected = sample_row("provider-1", "openai:chat", "deepseek-v4-pro");
        selected.endpoint_id = "endpoint-openai".to_string();
        selected.model_provider_model_name = "deepseek-v4-pro".to_string();
        selected.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
            name: "deepseek-v4-pro".to_string(),
            priority: 1,
            api_formats: None,
            endpoint_ids: Some(vec!["endpoint-openai".to_string()]),
            operations: None,
        }]);

        let mut scoped_out = selected.clone();
        scoped_out.provider_id = "provider-2".to_string();
        scoped_out.endpoint_id = "endpoint-claude".to_string();
        scoped_out.endpoint_api_format = "claude:messages".to_string();
        scoped_out.key_id = "key-provider-2".to_string();
        scoped_out.key_api_formats = Some(vec!["claude:messages".to_string()]);

        let repository =
            InMemoryMinimalCandidateSelectionReadRepository::seed(vec![scoped_out, selected]);

        let rows = repository
            .list_for_exact_api_format_and_requested_model("claude:messages", "deepseek-v4-pro")
            .await
            .expect("list should succeed");
        assert!(rows.is_empty());

        let rows = repository
            .list_for_exact_api_format_and_requested_model("openai:chat", "deepseek-v4-pro")
            .await
            .expect("list should succeed");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].endpoint_id, "endpoint-openai");
    }

    #[tokio::test]
    async fn requested_model_page_returns_requested_slice_only() {
        let rows = (0..5)
            .map(|index| sample_row(&format!("provider-{index}"), "openai:chat", "gpt-5"))
            .collect::<Vec<_>>();
        let repository = InMemoryMinimalCandidateSelectionReadRepository::seed(rows);

        let page = repository
            .list_for_exact_api_format_and_requested_model_page(
                &StoredRequestedModelCandidateRowsQuery {
                    api_format: "openai:chat".to_string(),
                    requested_model_name: "gpt-5".to_string(),
                    offset: 2,
                    limit: 2,
                },
            )
            .await
            .expect("page should load");

        assert_eq!(
            page.iter()
                .map(|row| row.provider_id.as_str())
                .collect::<Vec<_>>(),
            vec!["provider-2", "provider-3"]
        );
    }

    #[tokio::test]
    async fn filters_by_exact_api_format_only() {
        let repository = InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_row("provider-2", "openai:chat", "gpt-4.1"),
            sample_row("provider-1", "openai:chat", "gpt-4.1-mini"),
            sample_row("provider-3", "openai:responses", "gpt-4.1"),
        ]);

        let rows = repository
            .list_for_exact_api_format("openai:chat")
            .await
            .expect("list should succeed");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].provider_id, "provider-1");
        assert_eq!(rows[1].provider_id, "provider-2");
    }

    #[tokio::test]
    async fn lists_exact_api_format_in_stable_pages() {
        let repository = InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            sample_row("provider-3", "openai:chat", "gpt-4.1"),
            sample_row("provider-1", "openai:chat", "gpt-4.1"),
            sample_row("provider-2", "openai:chat", "gpt-4.1"),
        ]);

        let page = repository
            .list_for_exact_api_format_page(&StoredApiFormatCandidateRowsQuery {
                api_format: "openai:chat".to_string(),
                offset: 1,
                limit: 1,
            })
            .await
            .expect("API-format page should load");

        assert_eq!(page.len(), 1);
        assert_eq!(page[0].provider_id, "provider-2");
    }
}
