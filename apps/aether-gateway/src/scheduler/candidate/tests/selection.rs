use std::sync::Arc;
use std::time::Duration;

use aether_data::repository::candidate_selection::InMemoryMinimalCandidateSelectionReadRepository;
use aether_data::repository::candidates::InMemoryRequestCandidateRepository;
use aether_data::repository::provider_catalog::InMemoryProviderCatalogReadRepository;
use aether_data::repository::quota::InMemoryProviderQuotaRepository;
use aether_data_contracts::repository::candidate_selection::{
    StoredMinimalCandidateSelectionRow, StoredProviderModelMapping,
};
use aether_data_contracts::repository::candidates::{
    RequestCandidateStatus, StoredRequestCandidate,
};
use aether_data_contracts::repository::provider_catalog::StoredProviderCatalogKey;
use aether_data_contracts::repository::quota::StoredProviderQuotaSnapshot;
use aether_scheduler_core::{ClientSessionAffinity, SchedulerMinimalCandidateSelectionCandidate};
use serde_json::json;

use crate::cache::SchedulerAffinityTarget;
use crate::data::auth::GatewayAuthApiKeySnapshot;
use crate::data::candidate_selection::MinimalCandidateSelectionRowSource;
use crate::data::GatewayDataState;
use crate::{AppState, GatewayError};

use super::super::affinity::build_scheduler_affinity_cache_key;
use super::super::runtime::should_skip_provider_quota;
use super::super::selection::{
    collect_selectable_candidates as collect_selectable_candidates_impl,
    collect_selectable_candidates_with_skip_reasons as collect_selectable_candidates_with_skip_reasons_impl,
    is_exact_all_skipped_by_auth_limit, select_minimal_candidate as select_candidate_impl,
};
use super::support::{sample_auth_snapshot, sample_key, sample_provider, sample_row};

async fn select_candidate(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &AppState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Option<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    select_candidate_impl(
        selection_row_source,
        runtime_state,
        api_format,
        global_model_name,
        require_streaming,
        None,
        auth_snapshot,
        None,
        now_unix_secs,
        false,
    )
    .await
}

async fn collect_selectable_candidates(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &AppState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<Vec<SchedulerMinimalCandidateSelectionCandidate>, GatewayError> {
    collect_selectable_candidates_impl(
        selection_row_source,
        runtime_state,
        api_format,
        global_model_name,
        require_streaming,
        None,
        auth_snapshot,
        None,
        now_unix_secs,
        false,
    )
    .await
}

async fn collect_selectable_candidates_with_skip_reasons(
    selection_row_source: &(impl MinimalCandidateSelectionRowSource + Sync),
    runtime_state: &AppState,
    api_format: &str,
    global_model_name: &str,
    require_streaming: bool,
    auth_snapshot: Option<&GatewayAuthApiKeySnapshot>,
    now_unix_secs: u64,
) -> Result<
    (
        Vec<SchedulerMinimalCandidateSelectionCandidate>,
        Vec<super::super::SchedulerSkippedCandidate>,
    ),
    GatewayError,
> {
    collect_selectable_candidates_with_skip_reasons_impl(
        selection_row_source,
        runtime_state,
        api_format,
        global_model_name,
        require_streaming,
        None,
        auth_snapshot,
        None,
        now_unix_secs,
        false,
        None,
    )
    .await
}

fn provider_key_concurrency_row(
    provider_id: &str,
    endpoint_id: &str,
    key_id: &str,
    key_name: &str,
    key_priority: i32,
    _extra: i32,
) -> StoredMinimalCandidateSelectionRow {
    let mut row = sample_row();
    row.provider_id = provider_id.to_string();
    row.provider_name = provider_id.to_string();
    row.endpoint_id = endpoint_id.to_string();
    row.key_id = key_id.to_string();
    row.key_name = key_name.to_string();
    row
}

fn provider_key_with_concurrent_limit(
    key_id: &str,
    provider_id: &str,
    concurrent_limit: Option<i32>,
) -> StoredProviderCatalogKey {
    let mut key = sample_key(key_id, provider_id, Some(10));
    key.concurrent_limit = concurrent_limit;
    key
}

fn active_provider_key_candidate(
    candidate_id: &str,
    request_id: &str,
    provider_id: &str,
    endpoint_id: &str,
    key_id: &str,
    status: RequestCandidateStatus,
) -> StoredRequestCandidate {
    StoredRequestCandidate::new(
        candidate_id.to_string(),
        request_id.to_string(),
        None,
        None,
        None,
        None,
        0,
        0,
        Some(provider_id.to_string()),
        Some(endpoint_id.to_string()),
        Some(key_id.to_string()),
        status,
        None,
        false,
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        95_000,
        Some(95_000),
        None,
    )
    .expect("candidate should build")
}

fn provider_key_concurrency_state(
    rows: Vec<StoredMinimalCandidateSelectionRow>,
    keys: Vec<StoredProviderCatalogKey>,
    request_candidates: Vec<StoredRequestCandidate>,
) -> AppState {
    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(rows));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("test-provider-a", None),
            sample_provider("test-provider-b", None),
        ],
        Vec::new(),
        keys,
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(request_candidates));

    AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        )
}

#[test]
fn skips_only_exhausted_monthly_quota_provider() {
    let inactive = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "monthly_quota".to_string(),
        Some(10.0),
        1.0,
        Some(30),
        Some(1_000),
        None,
        false,
    )
    .expect("quota should build");
    assert!(!should_skip_provider_quota(&inactive, 2_000));

    let expired = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "monthly_quota".to_string(),
        Some(10.0),
        1.0,
        Some(30),
        Some(1_000),
        Some(1_500),
        true,
    )
    .expect("quota should build");
    assert!(!should_skip_provider_quota(&expired, 2_000));

    let exhausted = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "monthly_quota".to_string(),
        Some(10.0),
        10.0,
        Some(30),
        Some(1_000),
        None,
        true,
    )
    .expect("quota should build");
    assert!(should_skip_provider_quota(&exhausted, 2_000));

    let payg = StoredProviderQuotaSnapshot::new(
        "provider-1".to_string(),
        "pay_as_you_go".to_string(),
        None,
        10.0,
        None,
        None,
        None,
        true,
    )
    .expect("quota should build");
    assert!(!should_skip_provider_quota(&payg, 2_000));
}

#[tokio::test]
async fn provider_priority_mode_selection_is_deterministic_without_priority_data() {
    // 供应商侧优先级已裁剪：此路径候选恒同优先级，胜者由 seeded hash 决定。
    // 断言跨实例确定性——同一夹具的两份状态必须选出同一供应商。
    let build_state = || {
        let mut first = sample_row();
        first.provider_id = "provider-a".to_string();
        first.provider_name = "provider-a".to_string();
        first.endpoint_id = "endpoint-a".to_string();
        first.key_id = "key-a".to_string();
        first.key_name = "alpha".to_string();

        let mut second = sample_row();
        second.provider_id = "provider-b".to_string();
        second.provider_name = "provider-b".to_string();
        second.endpoint_id = "endpoint-b".to_string();
        second.key_id = "key-b".to_string();
        second.key_name = "beta".to_string();

        let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            first, second,
        ]));
        let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
        AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                    .with_system_config_values_for_tests(vec![(
                        "provider_priority_mode".to_string(),
                        json!("provider"),
                    )]),
            )
    };

    let state_first = build_state();
    let state_second = build_state();

    let selected_first = select_candidate(
        state_first.data.as_ref(),
        &state_first,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");
    let selected_second = select_candidate(
        state_second.data.as_ref(),
        &state_second,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected_first.provider_id, selected_second.provider_id);
    assert_eq!(selected_first.key_id, selected_second.key_id);
}

#[tokio::test]
async fn selects_by_global_key_priority_when_priority_mode_is_global_key() {
    let mut provider_first = sample_row();
    provider_first.provider_id = "provider-a".to_string();
    provider_first.provider_name = "provider-a".to_string();
    provider_first.endpoint_id = "endpoint-a".to_string();
    provider_first.key_id = "key-a".to_string();
    provider_first.key_name = "alpha".to_string();

    let mut global_key_first = sample_row();
    global_key_first.provider_id = "provider-b".to_string();
    global_key_first.provider_name = "provider-b".to_string();
    global_key_first.endpoint_id = "endpoint-b".to_string();
    global_key_first.key_id = "key-b".to_string();
    global_key_first.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        provider_first,
        global_key_first,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                .with_system_config_values_for_tests(vec![(
                    "provider_priority_mode".to_string(),
                    json!("global_key"),
                )]),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn scheduler_selection_prefers_required_capability_matches_before_priority_fallback() {
    let mut higher_priority_missing_capability = sample_row();
    higher_priority_missing_capability.provider_id = "provider-a".to_string();
    higher_priority_missing_capability.provider_name = "provider-a".to_string();
    higher_priority_missing_capability.endpoint_id = "endpoint-a".to_string();
    higher_priority_missing_capability.key_id = "key-a".to_string();
    higher_priority_missing_capability.key_name = "alpha".to_string();
    higher_priority_missing_capability.key_capabilities = Some(json!({"cache_1h": false}));

    let mut lower_priority_matching_capability = sample_row();
    lower_priority_matching_capability.provider_id = "provider-b".to_string();
    lower_priority_matching_capability.provider_name = "provider-b".to_string();
    lower_priority_matching_capability.endpoint_id = "endpoint-b".to_string();
    lower_priority_matching_capability.key_id = "key-b".to_string();
    lower_priority_matching_capability.key_name = "beta".to_string();
    lower_priority_matching_capability.key_capabilities = Some(json!({"cache_1h": true}));

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        higher_priority_missing_capability,
        lower_priority_matching_capability,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas),
        );
    let required_capabilities = json!({"cache_1h": true});

    let selected = select_candidate_impl(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&required_capabilities),
        None,
        None,
        100,
        false,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn fixed_order_ignores_cached_scheduler_affinity_promotion() {
    // fixed_order 不读取亲和缓存：播种 provider-b 亲和后，选择结果必须与
    // 未播种的基线完全一致。
    let build_state = |seed_affinity: bool| {
        let mut first = sample_row();
        first.provider_id = "provider-a".to_string();
        first.provider_name = "provider-a".to_string();
        first.endpoint_id = "endpoint-a".to_string();
        first.key_id = "key-a".to_string();
        first.key_name = "alpha".to_string();

        let mut second = sample_row();
        second.provider_id = "provider-b".to_string();
        second.provider_name = "provider-b".to_string();
        second.endpoint_id = "endpoint-b".to_string();
        second.key_id = "key-b".to_string();
        second.key_name = "beta".to_string();

        let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            first, second,
        ]));
        let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
        let state = AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                    .with_system_config_values_for_tests(vec![(
                        "scheduling_mode".to_string(),
                        json!("fixed_order"),
                    )]),
            );
        if seed_affinity {
            state.remember_scheduler_affinity_target(
                "scheduler_affinity:affinity-key-1:openai:chat:gpt-4.1",
                SchedulerAffinityTarget {
                    provider_id: "provider-b".to_string(),
                    endpoint_id: "endpoint-b".to_string(),
                    key_id: "key-b".to_string(),
                },
                Duration::from_secs(300),
                100,
            );
        }
        state
    };

    let auth_snapshot = sample_auth_snapshot("affinity-key-1");
    let baseline = select_candidate(
        build_state(false).data.as_ref(),
        &build_state(false),
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");
    let with_cache = select_candidate(
        build_state(true).data.as_ref(),
        &build_state(true),
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(baseline.provider_id, with_cache.provider_id);
    assert_eq!(baseline.key_id, with_cache.key_id);
}

#[tokio::test]
async fn fixed_order_selection_is_deterministic_for_equal_priorities() {
    // 同优先级（裁剪后恒等）下 fixed_order 依赖 seeded hash 平局，断言
    // 跨实例顺序确定性。
    let build_state = || {
        let mut first = sample_row();
        first.provider_id = "provider-a".to_string();
        first.provider_name = "provider-a".to_string();
        first.endpoint_id = "endpoint-a".to_string();
        first.key_id = "key-a".to_string();
        first.key_name = "alpha".to_string();

        let mut second = sample_row();
        second.provider_id = "provider-b".to_string();
        second.provider_name = "provider-b".to_string();
        second.endpoint_id = "endpoint-b".to_string();
        second.key_id = "key-b".to_string();
        second.key_name = "beta".to_string();

        let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
            first, second,
        ]));
        let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
        AppState::new()
            .expect("state should build")
            .with_data_state_for_tests(
                GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                    .with_system_config_values_for_tests(vec![(
                        "scheduling_mode".to_string(),
                        json!("fixed_order"),
                    )]),
            )
    };

    let auth_snapshot = sample_auth_snapshot("affinity-key-1");
    let first_run = collect_selectable_candidates(
        build_state().data.as_ref(),
        &build_state(),
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed");
    let second_run = collect_selectable_candidates(
        build_state().data.as_ref(),
        &build_state(),
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed");

    assert_eq!(first_run.len(), 2);
    let first_ids = first_run
        .iter()
        .map(|candidate| candidate.provider_id.as_str())
        .collect::<Vec<_>>();
    let second_ids = second_run
        .iter()
        .map(|candidate| candidate.provider_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(first_ids, second_ids);
}

#[tokio::test]
async fn cache_affinity_promotes_cached_scheduler_affinity_candidate_when_enabled() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "provider-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "provider-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                .with_system_config_values_for_tests(vec![(
                    "scheduling_mode".to_string(),
                    json!("cache_affinity"),
                )]),
        );

    let auth_snapshot = sample_auth_snapshot("affinity-key-1");
    let client_session_affinity = ClientSessionAffinity::from_session_key("session-1");
    let cache_key = build_scheduler_affinity_cache_key(
        Some(&auth_snapshot),
        "openai:chat",
        "gpt-4.1",
        Some(&client_session_affinity),
    )
    .expect("scheduler affinity cache key should build");
    state.remember_scheduler_affinity_target(
        &cache_key,
        SchedulerAffinityTarget {
            provider_id: "provider-b".to_string(),
            endpoint_id: "endpoint-b".to_string(),
            key_id: "key-b".to_string(),
        },
        Duration::from_secs(300),
        100,
    );

    let selected = select_candidate_impl(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        Some(&auth_snapshot),
        Some(&client_session_affinity),
        100,
        false,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn cache_affinity_ignores_cached_scheduler_affinity_without_client_session() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "provider-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "provider-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                .with_system_config_values_for_tests(vec![(
                    "scheduling_mode".to_string(),
                    json!("cache_affinity"),
                )]),
        );

    // 无客户端会话时亲和缓存不参与选择：播种 provider-b 的亲和后，结果
    // 必须与未播种的基线一致。
    let auth_snapshot = sample_auth_snapshot("affinity-key-1");
    let baseline_state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(
                Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
                    {
                        let mut first = sample_row();
                        first.provider_id = "provider-a".to_string();
                        first.provider_name = "provider-a".to_string();
                        first.endpoint_id = "endpoint-a".to_string();
                        first.key_id = "key-a".to_string();
                        first.key_name = "alpha".to_string();
                        first
                    },
                    {
                        let mut second = sample_row();
                        second.provider_id = "provider-b".to_string();
                        second.provider_name = "provider-b".to_string();
                        second.endpoint_id = "endpoint-b".to_string();
                        second.key_id = "key-b".to_string();
                        second.key_name = "beta".to_string();
                        second
                    },
                ])),
                Arc::new(InMemoryProviderQuotaRepository::seed(vec![])),
            )
            .with_system_config_values_for_tests(vec![(
                "scheduling_mode".to_string(),
                json!("cache_affinity"),
            )]),
        );

    let with_cache = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");
    let baseline = select_candidate(
        baseline_state.data.as_ref(),
        &baseline_state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(with_cache.provider_id, baseline.provider_id);
    assert_eq!(with_cache.key_id, baseline.key_id);
}

#[tokio::test]
async fn load_balance_selection_does_not_remember_scheduler_affinity() {
    let row = sample_row();
    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        row,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                .with_system_config_values_for_tests(vec![(
                    "scheduling_mode".to_string(),
                    json!("load_balance"),
                )]),
        );
    let auth_snapshot = sample_auth_snapshot("affinity-key-1");
    let client_session_affinity = ClientSessionAffinity::from_session_key("session-1");
    let cache_key = build_scheduler_affinity_cache_key(
        Some(&auth_snapshot),
        "openai:chat",
        "gpt-4.1",
        Some(&client_session_affinity),
    )
    .expect("scheduler affinity cache key should build");

    let selected = select_candidate_impl(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        Some(&auth_snapshot),
        Some(&client_session_affinity),
        100,
        false,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.key_id, "key-1");
    // load_balance 已软删除并映射为 cache_affinity：带客户端会话的选择
    // 现在按 cache_affinity 语义记住亲和目标。
    assert!(state
        .read_scheduler_affinity_target(cache_key.as_str(), Duration::from_secs(300))
        .is_some());
}

#[tokio::test]
async fn load_balance_ignores_provider_priority_and_cached_affinity() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "provider-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "provider-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas)
                .with_system_config_values_for_tests(vec![(
                    "scheduling_mode".to_string(),
                    json!("load_balance"),
                )]),
        );

    let auth_snapshot = sample_auth_snapshot("affinity-key-1");
    state.remember_scheduler_affinity_target(
        "scheduler_affinity:affinity-key-1:openai:chat:gpt-4.1",
        SchedulerAffinityTarget {
            provider_id: "provider-b".to_string(),
            endpoint_id: "endpoint-b".to_string(),
            key_id: "key-b".to_string(),
        },
        Duration::from_secs(300),
        100,
    );

    let first_pass = collect_selectable_candidates(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("first pass should succeed");
    let mut provider_b_first_seed = None;
    for seed in 101..600 {
        let pass = collect_selectable_candidates(
            state.data.as_ref(),
            &state,
            "openai:chat",
            "gpt-4.1",
            false,
            Some(&auth_snapshot),
            seed,
        )
        .await
        .expect("seeded pass should succeed");
        if pass
            .first()
            .is_some_and(|candidate| candidate.provider_id == "provider-b")
        {
            provider_b_first_seed = Some(seed);
            break;
        }
    }
    let provider_b_first_seed = provider_b_first_seed
        .expect("test seed should allow provider-b to win despite lower priority");
    let second_pass = collect_selectable_candidates(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        provider_b_first_seed,
    )
    .await
    .expect("second pass should succeed");

    assert_eq!(first_pass.len(), 2);
    assert_eq!(second_pass.len(), 2);
    assert_eq!(second_pass[0].provider_id, "provider-b");
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_quota_is_exhausted() {
    let mut first = sample_row();
    first.provider_id = "provider-1".to_string();
    first.provider_name = "openai-primary".to_string();
    first.endpoint_id = "endpoint-1".to_string();
    first.key_id = "key-1".to_string();
    first.key_name = "primary".to_string();
    first.model_provider_model_name = "gpt-4.1-primary".to_string();
    first.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-primary".to_string(),
        priority: 1,
        api_formats: None,
        endpoint_ids: None,
        operations: None,
    }]);

    let mut second = sample_row();
    second.provider_id = "provider-2".to_string();
    second.provider_name = "openai-secondary".to_string();
    second.endpoint_id = "endpoint-2".to_string();
    second.key_id = "key-2".to_string();
    second.key_name = "secondary".to_string();
    second.model_provider_model_name = "gpt-4.1-secondary".to_string();
    second.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-secondary".to_string(),
        priority: 1,
        api_formats: None,
        endpoint_ids: None,
        operations: None,
    }]);

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![
        StoredProviderQuotaSnapshot::new(
            "provider-1".to_string(),
            "monthly_quota".to_string(),
            Some(10.0),
            10.0,
            Some(30),
            Some(1_000),
            None,
            true,
        )
        .expect("quota should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_and_quota_for_tests(candidates, quotas),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        2_000,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-2");
    assert_eq!(selected.selected_provider_model_name, "gpt-4.1-secondary");
}

#[tokio::test]
async fn cooled_down_when_recent_failures_are_recorded_for_same_key() {
    let mut first = sample_row();
    first.provider_id = "provider-1".to_string();
    first.provider_name = "openai-primary".to_string();
    first.endpoint_id = "endpoint-1".to_string();
    first.key_id = "key-1".to_string();
    first.key_name = "primary".to_string();
    first.model_provider_model_name = "gpt-4.1-primary".to_string();
    first.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-primary".to_string(),
        priority: 1,
        api_formats: None,
        endpoint_ids: None,
        operations: None,
    }]);

    let mut second = sample_row();
    second.provider_id = "provider-2".to_string();
    second.provider_name = "openai-secondary".to_string();
    second.endpoint_id = "endpoint-2".to_string();
    second.key_id = "key-2".to_string();
    second.key_name = "secondary".to_string();
    second.model_provider_model_name = "gpt-4.1-secondary".to_string();
    second.model_provider_model_mappings = Some(vec![StoredProviderModelMapping {
        name: "gpt-4.1-secondary".to_string(),
        priority: 1,
        api_formats: None,
        endpoint_ids: None,
        operations: None,
    }]);

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Failed,
            None,
            false,
            Some(502),
            None,
            Some("upstream".to_string()),
            Some(100),
            None,
            None,
            None,
            95_000,
            Some(95_000),
            Some(95_000),
        )
        .expect("candidate should build"),
        StoredRequestCandidate::new(
            "cand-2".to_string(),
            "req-2".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Cancelled,
            None,
            false,
            Some(499),
            None,
            Some("cancelled".to_string()),
            Some(80),
            None,
            None,
            None,
            98_000,
            Some(98_000),
            Some(98_000),
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_quota_and_request_candidates_for_tests(
                candidates,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-2");
    assert_eq!(selected.selected_provider_model_name, "gpt-4.1-secondary");
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_concurrent_limit_is_reached() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", Some(1)),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        Vec::new(),
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            None,
            None,
            None,
            0,
            0,
            Some("provider-a".to_string()),
            Some("endpoint-a".to_string()),
            Some("key-a".to_string()),
            RequestCandidateStatus::Streaming,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            95_000,
            Some(95_000),
            None,
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn provider_key_concurrency_selects_next_key_when_first_provider_key_concurrent_limit_is_reached(
) {
    let state = provider_key_concurrency_state(
        vec![
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-a",
                "alpha",
                0,
                0,
            ),
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-b",
                "beta",
                0,
                1,
            ),
        ],
        vec![
            provider_key_with_concurrent_limit("provider-key-a", "test-provider-a", Some(1)),
            provider_key_with_concurrent_limit("provider-key-b", "test-provider-a", Some(1)),
        ],
        vec![active_provider_key_candidate(
            "cand-provider-key-a",
            "req-provider-key-a",
            "test-provider-a",
            "endpoint-a",
            "provider-key-a",
            RequestCandidateStatus::Streaming,
        )],
    );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "test-provider-a");
    assert_eq!(selected.key_id, "provider-key-b");
}

#[tokio::test]
async fn provider_key_concurrency_selects_next_provider_when_all_provider_keys_concurrent_limit_reached(
) {
    let state = provider_key_concurrency_state(
        vec![
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-a",
                "alpha",
                0,
                0,
            ),
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-b",
                "beta",
                0,
                1,
            ),
            provider_key_concurrency_row(
                "test-provider-b",
                "endpoint-b",
                "provider-key-c",
                "gamma",
                1,
                0,
            ),
        ],
        vec![
            provider_key_with_concurrent_limit("provider-key-a", "test-provider-a", Some(1)),
            provider_key_with_concurrent_limit("provider-key-b", "test-provider-a", Some(1)),
            provider_key_with_concurrent_limit("provider-key-c", "test-provider-b", Some(1)),
        ],
        vec![
            active_provider_key_candidate(
                "cand-provider-key-a",
                "req-provider-key-a",
                "test-provider-a",
                "endpoint-a",
                "provider-key-a",
                RequestCandidateStatus::Pending,
            ),
            active_provider_key_candidate(
                "cand-provider-key-b",
                "req-provider-key-b",
                "test-provider-a",
                "endpoint-a",
                "provider-key-b",
                RequestCandidateStatus::Streaming,
            ),
        ],
    );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "test-provider-b");
    assert_eq!(selected.key_id, "provider-key-c");
}

#[tokio::test]
async fn provider_key_concurrency_returns_none_when_all_provider_keys_concurrent_limit_reached() {
    let state = provider_key_concurrency_state(
        vec![
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-a",
                "alpha",
                0,
                0,
            ),
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-b",
                "beta",
                0,
                1,
            ),
            provider_key_concurrency_row(
                "test-provider-b",
                "endpoint-b",
                "provider-key-c",
                "gamma",
                1,
                0,
            ),
        ],
        vec![
            provider_key_with_concurrent_limit("provider-key-a", "test-provider-a", Some(1)),
            provider_key_with_concurrent_limit("provider-key-b", "test-provider-a", Some(1)),
            provider_key_with_concurrent_limit("provider-key-c", "test-provider-b", Some(1)),
        ],
        vec![
            active_provider_key_candidate(
                "cand-provider-key-a",
                "req-provider-key-a",
                "test-provider-a",
                "endpoint-a",
                "provider-key-a",
                RequestCandidateStatus::Pending,
            ),
            active_provider_key_candidate(
                "cand-provider-key-b",
                "req-provider-key-b",
                "test-provider-a",
                "endpoint-a",
                "provider-key-b",
                RequestCandidateStatus::Streaming,
            ),
            active_provider_key_candidate(
                "cand-provider-key-c",
                "req-provider-key-c",
                "test-provider-b",
                "endpoint-b",
                "provider-key-c",
                RequestCandidateStatus::Streaming,
            ),
        ],
    );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed");

    assert!(selected.is_none());

    let (selected_candidates, skipped_candidates) =
        collect_selectable_candidates_with_skip_reasons(
            state.data.as_ref(),
            &state,
            "openai:chat",
            "gpt-4.1",
            false,
            None,
            100,
        )
        .await
        .expect("selection should succeed");

    assert!(selected_candidates.is_empty());
    assert_eq!(skipped_candidates.len(), 3);
    assert!(skipped_candidates
        .iter()
        .all(|skipped| { skipped.skip_reason == "provider_key_concurrency_limit_reached" }));
    assert!(!is_exact_all_skipped_by_auth_limit(
        &selected_candidates,
        &skipped_candidates,
    ));
}

#[tokio::test]
async fn provider_key_concurrency_collects_exact_skip_reason_for_saturated_provider_keys() {
    let state = provider_key_concurrency_state(
        vec![
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-a",
                "alpha",
                0,
                0,
            ),
            provider_key_concurrency_row(
                "test-provider-a",
                "endpoint-a",
                "provider-key-b",
                "beta",
                0,
                1,
            ),
        ],
        vec![
            provider_key_with_concurrent_limit("provider-key-a", "test-provider-a", Some(1)),
            provider_key_with_concurrent_limit("provider-key-b", "test-provider-a", Some(1)),
        ],
        vec![active_provider_key_candidate(
            "cand-provider-key-a",
            "req-provider-key-a",
            "test-provider-a",
            "endpoint-a",
            "provider-key-a",
            RequestCandidateStatus::Pending,
        )],
    );

    let (selected_candidates, skipped_candidates) =
        collect_selectable_candidates_with_skip_reasons(
            state.data.as_ref(),
            &state,
            "openai:chat",
            "gpt-4.1",
            false,
            None,
            100,
        )
        .await
        .expect("selection should succeed");

    assert_eq!(selected_candidates.len(), 1);
    assert_eq!(selected_candidates[0].provider_id, "test-provider-a");
    assert_eq!(selected_candidates[0].key_id, "provider-key-b");
    assert_eq!(skipped_candidates.len(), 1);
    assert_eq!(
        skipped_candidates[0].candidate.provider_id,
        "test-provider-a"
    );
    assert_eq!(skipped_candidates[0].candidate.key_id, "provider-key-a");
    assert_eq!(
        skipped_candidates[0].skip_reason,
        "provider_key_concurrency_limit_reached",
    );
}

#[tokio::test]
async fn returns_none_when_auth_api_key_concurrent_limit_is_reached() {
    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        sample_row(),
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![sample_provider("provider-1", None)],
        Vec::new(),
        Vec::new(),
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            Some("user-1".to_string()),
            Some("api-key-1".to_string()),
            None,
            None,
            0,
            0,
            Some("provider-1".to_string()),
            Some("endpoint-1".to_string()),
            Some("key-1".to_string()),
            RequestCandidateStatus::Pending,
            None,
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            95_000,
            Some(95_000),
            None,
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let mut auth_snapshot = sample_auth_snapshot("api-key-1");
    auth_snapshot.api_key_concurrent_limit = Some(1);

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed");

    assert!(selected.is_none());

    let (selected_candidates, skipped_candidates) =
        collect_selectable_candidates_with_skip_reasons(
            state.data.as_ref(),
            &state,
            "openai:chat",
            "gpt-4.1",
            false,
            Some(&auth_snapshot),
            100,
        )
        .await
        .expect("selection should succeed");
    assert!(is_exact_all_skipped_by_auth_limit(
        &selected_candidates,
        &skipped_candidates,
    ));
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_key_rpm_slots_are_reserved_for_new_user() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)),
            sample_key("key-b", "provider-b", Some(10)),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![
        StoredRequestCandidate::new(
            "cand-1".to_string(),
            "req-1".to_string(),
            None,
            Some("api-key-new-user".to_string()),
            None,
            None,
            0,
            0,
            Some("provider-a".to_string()),
            Some("endpoint-a".to_string()),
            Some("key-a".to_string()),
            RequestCandidateStatus::Success,
            None,
            false,
            Some(200),
            None,
            None,
            Some(10),
            Some(9),
            None,
            None,
            95_000,
            Some(95_000),
            Some(96_000),
        )
        .expect("candidate should build"),
    ]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let auth_snapshot = sample_auth_snapshot("api-key-new-user");

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        Some(&auth_snapshot),
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn selects_next_candidate_when_first_provider_key_circuit_is_open() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.2}})),
                Some(serde_json::json!({"openai:chat": {"open": true}})),
            ),
            sample_key("key-b", "provider-b", Some(10)),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn exposes_runtime_skipped_candidates_with_skip_reasons() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.2}})),
                Some(serde_json::json!({"openai:chat": {"open": true}})),
            ),
            sample_key("key-b", "provider-b", Some(10)),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let (selected, skipped) = collect_selectable_candidates_with_skip_reasons(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed");

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].provider_id, "provider-b");
    assert_eq!(skipped.len(), 1);
    assert_eq!(skipped[0].candidate.provider_id, "provider-a");
    assert_eq!(skipped[0].skip_reason, "key_circuit_open");
    assert!(!is_exact_all_skipped_by_auth_limit(&selected, &skipped));
}

#[tokio::test]
async fn same_priority_candidates_prefer_healthier_provider_key_before_id_order() {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.30}})),
                None,
            ),
            sample_key("key-b", "provider-b", Some(10)).with_health_fields(
                Some(serde_json::json!({"openai:chat": {"health_score": 0.95}})),
                None,
            ),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}

#[tokio::test]
async fn same_priority_candidates_use_aggregate_health_score_when_api_format_specific_health_is_missing(
) {
    let mut first = sample_row();
    first.provider_id = "provider-a".to_string();
    first.provider_name = "openai-a".to_string();
    first.endpoint_id = "endpoint-a".to_string();
    first.key_id = "key-a".to_string();
    first.key_name = "alpha".to_string();

    let mut second = sample_row();
    second.provider_id = "provider-b".to_string();
    second.provider_name = "openai-b".to_string();
    second.endpoint_id = "endpoint-b".to_string();
    second.key_id = "key-b".to_string();
    second.key_name = "beta".to_string();

    let candidates = Arc::new(InMemoryMinimalCandidateSelectionReadRepository::seed(vec![
        first, second,
    ]));
    let provider_catalog = Arc::new(InMemoryProviderCatalogReadRepository::seed(
        vec![
            sample_provider("provider-a", None),
            sample_provider("provider-b", None),
        ],
        Vec::new(),
        vec![
            sample_key("key-a", "provider-a", Some(10)).with_health_fields(
                Some(serde_json::json!({
                    "openai:responses": {"health_score": 0.40},
                    "claude:messages": {"health_score": 0.55}
                })),
                None,
            ),
            sample_key("key-b", "provider-b", Some(10)).with_health_fields(
                Some(serde_json::json!({
                    "openai:responses": {"health_score": 0.90},
                    "claude:messages": {"health_score": 0.92}
                })),
                None,
            ),
        ],
    ));
    let quotas = Arc::new(InMemoryProviderQuotaRepository::seed(vec![]));
    let request_candidates = Arc::new(InMemoryRequestCandidateRepository::seed(vec![]));
    let state = AppState::new()
        .expect("state should build")
        .with_data_state_for_tests(
            GatewayDataState::with_candidate_selection_provider_catalog_quota_and_request_candidates_for_tests(
                candidates,
                provider_catalog,
                quotas,
                request_candidates,
            ),
        );

    let selected = select_candidate(
        state.data.as_ref(),
        &state,
        "openai:chat",
        "gpt-4.1",
        false,
        None,
        100,
    )
    .await
    .expect("selection should succeed")
    .expect("candidate should exist");

    assert_eq!(selected.provider_id, "provider-b");
    assert_eq!(selected.key_id, "key-b");
}
