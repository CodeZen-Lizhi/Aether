use super::{
    any, build_router_with_state, build_state_with_execution_runtime_override,
    encrypt_python_fernet_plaintext, json, start_server, to_bytes, Arc, Body, Digest,
    InMemoryAuthApiKeySnapshotRepository, InMemoryMinimalCandidateSelectionReadRepository,
    InMemoryProviderCatalogReadRepository, InMemoryRequestCandidateRepository, Json, Mutex,
    Request, RequestCandidateReadRepository, RequestCandidateStatus, Router, Sha256, StatusCode,
    StoredAuthApiKeySnapshot, StoredMinimalCandidateSelectionRow, StoredProviderCatalogEndpoint,
    StoredProviderCatalogKey, StoredProviderCatalogProvider, StoredProviderModelMapping,
    DEVELOPMENT_ENCRYPTION_KEY, TRACE_ID_HEADER,
};
use aether_data::repository::usage::InMemoryUsageReadRepository;
use aether_data_contracts::repository::usage::{StoredRequestUsageAudit, UsageReadRepository};
use aether_usage_runtime::UsageRuntimeConfig;

const KIRO_CLAUDE_CLI_SYNC_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

fn run_kiro_claude_cli_sync_test<F, Fut>(test_name: &'static str, make_future: F)
where
    F: FnOnce() -> Fut + Send + 'static,
    Fut: std::future::Future<Output = ()> + 'static,
{
    let handle = std::thread::Builder::new()
        .name(test_name.to_string())
        .stack_size(KIRO_CLAUDE_CLI_SYNC_TEST_STACK_BYTES)
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("test runtime should build");
            runtime.block_on(make_future());
        })
        .expect("kiro claude cli sync test thread should spawn");

    if let Err(payload) = handle.join() {
        std::panic::resume_unwind(payload);
    }
}

async fn wait_for_completed_usage<T>(repository: &T, request_id: &str) -> StoredRequestUsageAudit
where
    T: UsageReadRepository + ?Sized,
{
    let timeout = std::time::Duration::from_secs(60);
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(usage) = repository
            .find_by_request_id(request_id)
            .await
            .expect("usage should read")
        {
            if usage.status == "completed" {
                return usage;
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "usage {request_id} should complete within {timeout:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

