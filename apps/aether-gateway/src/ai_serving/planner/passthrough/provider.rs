use axum::body::Body;
use axum::http::Response;
use std::collections::BTreeMap;
use url::form_urlencoded;

use aether_data_contracts::repository::candidates::{
    RequestCandidateStatus, UpsertRequestCandidateRecord,
};
use aether_scheduler_core::SchedulerMinimalCandidateSelectionCandidate;
use serde_json::{json, Value};
use tracing::warn;
use uuid::Uuid;

use crate::ai_serving::planner::common::{
    EXECUTION_RUNTIME_STREAM_DECISION_ACTION, EXECUTION_RUNTIME_SYNC_DECISION_ACTION,
};
use crate::ai_serving::planner::plan_builders::{AiStreamAttempt, AiSyncAttempt};
use crate::ai_serving::transport::auth::{
    build_openai_passthrough_headers, resolve_local_gemini_auth, resolve_local_standard_auth,
};
use crate::ai_serving::transport::policy::{
    supports_local_gemini_transport_with_network, supports_local_standard_transport_with_network,
};
use crate::ai_serving::transport::url::{
    build_claude_messages_url, build_gemini_content_url, build_passthrough_path_url,
};
use crate::ai_serving::transport::{
    apply_local_body_rules, apply_local_header_rules, build_passthrough_headers,
    ensure_upstream_auth_header, resolve_transport_execution_timeouts,
    resolve_transport_proxy_snapshot_with_tunnel_affinity,
};
use crate::ai_serving::{
    collect_control_headers, ConversionMode, ExecutionStrategy, GatewayControlDecision,
};
use crate::clock::current_unix_secs;
use crate::{
    append_execution_contract_fields_to_value, AiExecutionDecision, AppState, GatewayError,
};

mod family;
mod plans;
mod request;

pub(crate) use self::family::{
    build_local_same_format_provider_candidate_attempt_source,
    materialize_local_same_format_provider_candidate_attempts,
    maybe_build_local_same_format_provider_decision_payload_for_candidate,
    resolve_local_same_format_provider_decision_input, LocalSameFormatProviderCandidateAttempt,
    LocalSameFormatProviderCandidateAttemptSource, LocalSameFormatProviderDecisionInput,
    LocalSameFormatProviderFamily, LocalSameFormatProviderSpec,
};
pub(crate) use self::family::{
    maybe_build_pinned_stream_local_same_format_provider_decision_payload,
    maybe_build_stream_local_same_format_provider_decision_payload,
    maybe_build_sync_local_same_format_provider_decision_payload,
};
pub(crate) use self::plans::{
    build_local_stream_attempt_source, build_local_stream_plan_and_reports,
    build_local_sync_attempt_source, build_local_sync_plan_and_reports,
};

