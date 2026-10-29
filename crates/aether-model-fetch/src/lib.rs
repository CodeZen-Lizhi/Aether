mod config;
mod logic;
mod strategy;
mod transport;

pub use config::model_fetch_interval_minutes;
pub use logic::{
    aggregate_models_for_cache, build_models_fetch_url,
    deepseek_anthropic_models_fetch_uses_openai_auth, endpoint_supports_rust_models_fetch,
    extract_error_message, json_string_list, parse_models_response_page,
    selected_models_fetch_endpoints, upstream_metadata_namespace_updates, ModelsFetchPage,
};
pub use strategy::{fetch_models_from_transports, ModelsFetchOutcome};
pub use transport::{
    build_models_fetch_execution_plan, build_standard_models_fetch_execution_plan,
    ModelFetchTransportRuntime,
};
