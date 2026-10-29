use std::time::Duration;

use aether_contracts::{ExecutionPlan, ExecutionResult, ProxySnapshot};
use aether_data_contracts::repository::candidates::{
    KeyAttemptUsage, StoredRequestCandidate, UpsertRequestCandidateRecord,
};
use aether_data_contracts::repository::global_models::{
    AdminGlobalModelListQuery, AdminProviderModelListQuery, StoredAdminGlobalModelPage,
    StoredAdminProviderModel, UpsertAdminProviderModelRecord,
};
use aether_data_contracts::repository::provider_catalog::{
    StoredProviderCatalogKey, StoredProviderCatalogProvider,
};
use aether_data_contracts::repository::quota::StoredProviderQuotaSnapshot;
use aether_model_fetch::ModelFetchTransportRuntime;
use aether_scheduler_core::SchedulerAffinityTarget;
use async_trait::async_trait;
use serde_json::Value;

use super::{AppState, GatewayError};
use crate::clock::current_unix_secs;
use crate::provider_transport::GatewayProviderTransportSnapshot;
use crate::request_candidate_runtime::{
    RequestCandidateRuntimeCapabilityReader, RequestCandidateRuntimeReader,
    RequestCandidateRuntimeWriter,
};
use crate::scheduler::state::SchedulerRuntimeState;
use crate::{execution_runtime, provider_transport};

const MODEL_FETCH_RESPONSE_BODY_LIMIT_BYTES: usize = 8 * 1024 * 1024;

#[async_trait]
impl provider_transport::TransportTunnelAffinityLookup for AppState {
    async fn lookup_tunnel_attachment_owner(
        &self,
        node_id: &str,
    ) -> Result<Option<provider_transport::TransportTunnelAttachmentOwner>, String> {
        self.tunnel
            .lookup_attachment_owner(self.data.as_ref(), node_id)
            .await
            .map(|owner| {
                owner.map(|owner| provider_transport::TransportTunnelAttachmentOwner {
                    gateway_instance_id: owner.gateway_instance_id,
                    relay_base_url: owner.relay_base_url,
                    observed_at_unix_secs: owner.observed_at_unix_secs,
                })
            })
    }
}

#[async_trait]
impl provider_transport::VideoTaskTransportSnapshotLookup for AppState {
    async fn read_video_task_provider_transport_snapshot(
        &self,
        provider_id: &str,
        endpoint_id: &str,
        key_id: &str,
    ) -> Result<Option<GatewayProviderTransportSnapshot>, String> {
        self.read_provider_transport_snapshot(provider_id, endpoint_id, key_id)
            .await
            .map_err(GatewayError::into_message)
    }
}

#[async_trait]
impl ModelFetchTransportRuntime for AppState {
    async fn resolve_model_fetch_proxy(
        &self,
        transport: &GatewayProviderTransportSnapshot,
    ) -> Option<ProxySnapshot> {
        self.resolve_transport_proxy_snapshot_with_tunnel_affinity(transport)
            .await
    }

    async fn execute_model_fetch_execution_plan(
        &self,
        plan: &ExecutionPlan,
    ) -> Result<ExecutionResult, String> {
        let bounded_plan = execution_runtime::transport::with_upstream_response_body_limit(
            plan,
            MODEL_FETCH_RESPONSE_BODY_LIMIT_BYTES,
        );
        execution_runtime::execute_execution_runtime_sync_plan(self, None, &bounded_plan)
            .await
            .map_err(GatewayError::into_message)
    }
}

#[async_trait]
impl RequestCandidateRuntimeReader for AppState {
    async fn read_request_candidates_by_request_id(
        &self,
        request_id: &str,
    ) -> Result<Vec<StoredRequestCandidate>, GatewayError> {
        AppState::read_request_candidates_by_request_id(self, request_id).await
    }
}

#[async_trait]
impl RequestCandidateRuntimeCapabilityReader for AppState {
    async fn read_request_candidate_user_model_capability_settings(
        &self,
        user_id: &str,
    ) -> Result<Option<Value>, GatewayError> {
        AppState::read_user_model_capability_settings(self, user_id).await
    }

    async fn read_request_candidate_api_key_force_capabilities(
        &self,
        user_id: &str,
        api_key_id: &str,
    ) -> Result<Option<Value>, GatewayError> {
        AppState::read_auth_api_key_force_capabilities(self, user_id, api_key_id).await
    }
}

#[async_trait]
impl RequestCandidateRuntimeWriter for AppState {
    fn has_request_candidate_data_writer(&self) -> bool {
        AppState::has_request_candidate_data_writer(self)
    }

    async fn upsert_request_candidate(
        &self,
        candidate: UpsertRequestCandidateRecord,
    ) -> Result<Option<StoredRequestCandidate>, GatewayError> {
        AppState::upsert_request_candidate(self, candidate).await
    }

    async fn enqueue_request_candidate_status(
        &self,
        candidate: UpsertRequestCandidateRecord,
    ) -> Result<Option<()>, GatewayError> {
        AppState::enqueue_request_candidate_status(self, candidate).await
    }

    fn try_enqueue_request_candidate_status(
        &self,
        candidate: UpsertRequestCandidateRecord,
    ) -> Result<(), UpsertRequestCandidateRecord> {
        AppState::try_enqueue_request_candidate_status(self, candidate)
    }
}

#[async_trait]
impl SchedulerRuntimeState for AppState {
    async fn read_provider_quota_snapshot(
        &self,
        provider_id: &str,
    ) -> Result<Option<StoredProviderQuotaSnapshot>, GatewayError> {
        AppState::read_provider_quota_snapshot(self, provider_id).await
    }

    async fn read_provider_catalog_providers_by_ids(
        &self,
        provider_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogProvider>, GatewayError> {
        AppState::read_provider_catalog_providers_by_ids(self, provider_ids).await
    }

    async fn read_provider_catalog_keys_by_ids(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, GatewayError> {
        AppState::read_provider_catalog_keys_by_ids(self, key_ids).await
    }

    async fn read_provider_catalog_keys_by_ids_strong(
        &self,
        key_ids: &[String],
    ) -> Result<Vec<StoredProviderCatalogKey>, GatewayError> {
        AppState::read_provider_catalog_keys_by_ids_strong(self, key_ids).await
    }

    async fn read_recent_request_candidates(
        &self,
        limit: usize,
    ) -> Result<Vec<StoredRequestCandidate>, GatewayError> {
        AppState::read_recent_request_candidates(self, limit).await
    }

    async fn aggregate_attempted_request_candidate_usage_by_key_since(
        &self,
        min_observed_at_unix_secs_by_key: &std::collections::BTreeMap<String, u64>,
    ) -> Result<Vec<KeyAttemptUsage>, GatewayError> {
        AppState::aggregate_attempted_request_candidate_usage_by_key_since(
            self,
            min_observed_at_unix_secs_by_key,
        )
        .await
    }

    fn provider_key_rpm_reset_at(&self, key_id: &str, now_unix_secs: u64) -> Option<u64> {
        AppState::provider_key_rpm_reset_at(self, key_id, now_unix_secs)
    }

    fn read_cached_scheduler_affinity_target(
        &self,
        cache_key: &str,
        ttl: Duration,
    ) -> Option<SchedulerAffinityTarget> {
        AppState::read_scheduler_affinity_target(self, cache_key, ttl)
    }

    fn scheduler_affinity_epoch(&self) -> u64 {
        AppState::scheduler_affinity_epoch(self)
    }

    fn remember_scheduler_affinity_target(
        &self,
        cache_key: &str,
        target: SchedulerAffinityTarget,
        ttl: Duration,
        max_entries: usize,
    ) {
        AppState::remember_scheduler_affinity_target(self, cache_key, target, ttl, max_entries);
    }

    fn remember_scheduler_affinity_target_for_epoch(
        &self,
        cache_key: &str,
        target: SchedulerAffinityTarget,
        ttl: Duration,
        max_entries: usize,
        expected_epoch: Option<u64>,
    ) -> bool {
        AppState::remember_scheduler_affinity_target_for_epoch(
            self,
            cache_key,
            target,
            ttl,
            max_entries,
            expected_epoch,
        )
    }

    async fn read_scheduler_ordering_config(
        &self,
    ) -> Result<crate::scheduler::config::SchedulerOrderingConfig, GatewayError> {
        crate::scheduler::config::read_scheduler_ordering_config(self).await
    }
}
