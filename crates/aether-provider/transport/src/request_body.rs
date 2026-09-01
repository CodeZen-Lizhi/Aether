use serde_json::Value;

use crate::snapshot::GatewayProviderTransportSnapshot;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportRequestBodySemanticsError {
    message: &'static str,
}

impl TransportRequestBodySemanticsError {
    pub const fn message(&self) -> &'static str {
        self.message
    }
}

impl std::fmt::Display for TransportRequestBodySemanticsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.message)
    }
}

impl std::error::Error for TransportRequestBodySemanticsError {}

/// Provider-specific request body semantics that must be applied after local
/// conversion. With the fixed provider types removed, custom API-key providers
/// pass bodies through unchanged and this hook is currently a no-op.
pub fn apply_transport_request_body_semantics(
    _provider_request_body: &mut Value,
    _transport: &GatewayProviderTransportSnapshot,
    _provider_api_format: &str,
) -> Result<(), TransportRequestBodySemanticsError> {
    Ok(())
}
