#[cfg(test)]
use axum::http::Uri;

mod auth;
mod execute;
mod public;
mod route;

pub(crate) use auth::{
    execution_plan_balance_capacity_rejection, extract_requested_model,
    refresh_execution_runtime_auth_context, refresh_execution_runtime_auth_context_with_snapshot,
    request_model_local_rejection, resolve_execution_runtime_auth_context,
    should_buffer_request_for_local_auth, trusted_auth_local_rejection,
    GatewayAdminPrincipalContext, GatewayControlAuthContext, GatewayCredentialCarrier,
    GatewayLocalAuthRejection,
};
pub(crate) use execute::{allows_control_execute_emergency, maybe_execute_via_control};
pub(crate) use public::{resolve_public_request_context, GatewayPublicRequestContext};
#[cfg(test)]
pub(crate) use route::classify_control_route;
pub(crate) use route::{resolve_control_route, GatewayControlDecision};

#[cfg(test)]
mod tests;
