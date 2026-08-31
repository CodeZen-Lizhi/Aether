use std::borrow::Cow;

use serde_json::Value;

use crate::ai_serving::ExecutionRuntimeAuthContext;
use crate::{AppState, GatewayError};

pub(crate) struct ProviderRequestRedaction<'a> {
    pub(crate) body_json: Cow<'a, Value>,
    pub(crate) redacted: bool,
}

impl<'a> ProviderRequestRedaction<'a> {
    fn disabled(body_json: &'a Value) -> Self {
        Self {
            body_json: Cow::Borrowed(body_json),
            redacted: false,
        }
    }
}

pub(crate) fn request_identity_response_encoding_when_redacted(
    _headers: &mut std::collections::BTreeMap<String, String>,
    _redacted: bool,
) {
}

pub(crate) fn sanitize_upstream_url_for_log(raw: &str) -> String {
    if let Ok(mut url) = url::Url::parse(raw) {
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return "<invalid-upstream-url>".to_string();
        }
        let _ = url.set_username("");
        let _ = url.set_password(None);
        url.set_query(None);
        url.set_fragment(None);
        return url.to_string();
    }

    let suffix_offset = raw
        .char_indices()
        .find_map(|(offset, character)| matches!(character, '?' | '#').then_some(offset))
        .unwrap_or(raw.len());
    let path = &raw[..suffix_offset];
    if path.starts_with('/') && !path.starts_with("//") && !path.contains('@') {
        path.to_string()
    } else {
        "<invalid-upstream-url>".to_string()
    }
}

/// Chat PII redaction was removed from this personal build; the planner always
/// passes the request body through untouched.
pub(crate) async fn resolve_provider_chat_pii_redaction<'a>(
    _state: &AppState,
    _parts: &http::request::Parts,
    body_json: &'a Value,
    _auth_context: &ExecutionRuntimeAuthContext,
    _client_api_format: &str,
    _reasoning_replay_policy: crate::ai_serving::OpenAiResponsesReasoningReplayPolicy,
    _candidate_id: &str,
) -> Result<ProviderRequestRedaction<'a>, GatewayError> {
    Ok(ProviderRequestRedaction::disabled(body_json))
}
