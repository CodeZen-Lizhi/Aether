use serde_json::Value;

use crate::formats::openai::image::stream::{OpenAiImageChatStreamState, OpenAiImageStreamState};
use crate::formats::openai::responses::history::ResponseHistoryRecord;
use crate::formats::openai::responses::response::ensure_modern_openai_responses_response_fields;
use crate::formats::shared::model_directives::model_directive_display_model_from_report_context;
use crate::formats::shared::stream_core::StreamingStandardFormatMatrix;
use crate::formats::shared::AiSurfaceFinalizeError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FinalizeStreamRewriteMode {
    ModelDirectiveDisplay,
    OpenAiResponsesCompat,
    OpenAiImage,
    OpenAiImageToOpenAiChat,
    Standard,
}

pub fn resolve_finalize_stream_rewrite_mode(
    report_context: &Value,
) -> Option<FinalizeStreamRewriteMode> {
    let needs_conversion = report_context
        .get("needs_conversion")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let provider_api_format = report_context
        .get("provider_api_format")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let provider_stream_event_api_format = report_context
        .get("provider_stream_event_api_format")
        .or_else(|| report_context.get("provider_stream_api_format"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| provider_api_format.clone());
    let client_api_format = report_context
        .get("client_api_format")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    let stream_needs_conversion = needs_conversion
        || !is_same_format_family(
            provider_stream_event_api_format.as_str(),
            client_api_format.as_str(),
        );

    if provider_api_format == "openai:image" && client_api_format == "openai:chat" {
        return Some(FinalizeStreamRewriteMode::OpenAiImageToOpenAiChat);
    }

    if provider_api_format == "openai:image" && client_api_format == "openai:image" {
        return Some(FinalizeStreamRewriteMode::OpenAiImage);
    }

    if stream_needs_conversion {
        // CPA strategy: when provider and client share the same wire format
        // (exact match or same family), pass through the stream verbatim.
        // Parsing→rebuilding only adds overhead and may lose information
        // (encrypted_content, original item IDs, etc.).
        if is_same_format_family(
            provider_stream_event_api_format.as_str(),
            client_api_format.as_str(),
        ) {
            if is_openai_responses_family(provider_stream_event_api_format.as_str())
                && is_openai_responses_family(client_api_format.as_str())
            {
                return Some(FinalizeStreamRewriteMode::OpenAiResponsesCompat);
            }
            return model_directive_display_model_from_report_context(report_context)
                .map(|_| FinalizeStreamRewriteMode::ModelDirectiveDisplay);
        }
        return supports_standard_stream_rewrite(
            provider_stream_event_api_format.as_str(),
            client_api_format.as_str(),
        )
        .then_some(FinalizeStreamRewriteMode::Standard);
    }

    if model_directive_display_model_from_report_context(report_context).is_some()
        && provider_stream_event_api_format == client_api_format
        && is_standard_provider_api_format(provider_stream_event_api_format.as_str())
    {
        return Some(FinalizeStreamRewriteMode::ModelDirectiveDisplay);
    }

    if is_same_format_family(
        provider_stream_event_api_format.as_str(),
        client_api_format.as_str(),
    ) && is_openai_responses_family(provider_stream_event_api_format.as_str())
        && is_openai_responses_family(client_api_format.as_str())
    {
        return Some(FinalizeStreamRewriteMode::OpenAiResponsesCompat);
    }

    None
}

enum AiSurfaceStreamRewriteState {
    ModelDirectiveDisplay,
    OpenAiResponsesCompat,
    OpenAiImage(Box<OpenAiImageStreamState>),
    OpenAiImageToOpenAiChat(Box<OpenAiImageChatStreamState>),
    Standard(Box<StreamingStandardFormatMatrix>),
}

pub struct AiSurfaceStreamRewriter<'a> {
    report_context: &'a Value,
    buffered: Vec<u8>,
    state: AiSurfaceStreamRewriteState,
}

pub fn maybe_build_ai_surface_stream_rewriter<'a>(
    report_context: Option<&'a Value>,
) -> Option<AiSurfaceStreamRewriter<'a>> {
    let report_context = report_context?;
    let state = match resolve_finalize_stream_rewrite_mode(report_context)? {
        FinalizeStreamRewriteMode::ModelDirectiveDisplay => {
            AiSurfaceStreamRewriteState::ModelDirectiveDisplay
        }
        FinalizeStreamRewriteMode::OpenAiResponsesCompat => {
            AiSurfaceStreamRewriteState::OpenAiResponsesCompat
        }
        FinalizeStreamRewriteMode::OpenAiImage => {
            AiSurfaceStreamRewriteState::OpenAiImage(Box::<OpenAiImageStreamState>::default())
        }
        FinalizeStreamRewriteMode::OpenAiImageToOpenAiChat => {
            AiSurfaceStreamRewriteState::OpenAiImageToOpenAiChat(
                Box::<OpenAiImageChatStreamState>::default(),
            )
        }
        FinalizeStreamRewriteMode::Standard => {
            AiSurfaceStreamRewriteState::Standard(Box::<StreamingStandardFormatMatrix>::default())
        }
    };

    Some(AiSurfaceStreamRewriter {
        report_context,
        buffered: Vec::new(),
        state,
    })
}

impl AiSurfaceStreamRewriter<'_> {
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<Vec<u8>, AiSurfaceFinalizeError> {
        match &mut self.state {
            AiSurfaceStreamRewriteState::OpenAiImage(state) => {
                state.push_chunk(self.report_context, chunk)
            }
            AiSurfaceStreamRewriteState::OpenAiImageToOpenAiChat(state) => {
                state.push_chunk(self.report_context, chunk)
            }
            AiSurfaceStreamRewriteState::ModelDirectiveDisplay
            | AiSurfaceStreamRewriteState::OpenAiResponsesCompat
            | AiSurfaceStreamRewriteState::Standard(_) => {
                self.buffered.extend_from_slice(chunk);
                let mut output = Vec::new();
                while let Some(line_end) = self.buffered.iter().position(|byte| *byte == b'\n') {
                    let line = self.buffered.drain(..=line_end).collect::<Vec<_>>();
                    output.extend(self.transform_line(line)?);
                }
                Ok(output)
            }
        }
    }

    pub fn finish(&mut self) -> Result<Vec<u8>, AiSurfaceFinalizeError> {
        match &mut self.state {
            AiSurfaceStreamRewriteState::OpenAiImage(state) => state.finish(self.report_context),
            AiSurfaceStreamRewriteState::OpenAiImageToOpenAiChat(state) => {
                state.finish(self.report_context)
            }
            AiSurfaceStreamRewriteState::ModelDirectiveDisplay
            | AiSurfaceStreamRewriteState::OpenAiResponsesCompat
            | AiSurfaceStreamRewriteState::Standard(_) => {
                if self.buffered.is_empty() {
                    if let AiSurfaceStreamRewriteState::Standard(state) = &mut self.state {
                        return state.finish(self.report_context);
                    }
                    return Ok(Vec::new());
                }
                let line = std::mem::take(&mut self.buffered);
                let mut output = self.transform_line(line)?;
                if let AiSurfaceStreamRewriteState::Standard(state) = &mut self.state {
                    output.extend(state.finish(self.report_context)?);
                }
                Ok(output)
            }
        }
    }

    pub fn take_response_history_record(&mut self) -> Option<ResponseHistoryRecord> {
        match &mut self.state {
            AiSurfaceStreamRewriteState::Standard(state) => state.take_response_history_record(),
            _ => None,
        }
    }

    fn transform_line(&mut self, line: Vec<u8>) -> Result<Vec<u8>, AiSurfaceFinalizeError> {
        match &mut self.state {
            AiSurfaceStreamRewriteState::ModelDirectiveDisplay => {
                rewrite_model_directive_stream_line(self.report_context, line)
            }
            AiSurfaceStreamRewriteState::OpenAiResponsesCompat => {
                rewrite_openai_responses_compat_stream_line(self.report_context, line)
            }
            AiSurfaceStreamRewriteState::Standard(state) => {
                transform_standard_line(state, self.report_context, line)
            }
            AiSurfaceStreamRewriteState::OpenAiImage(_)
            | AiSurfaceStreamRewriteState::OpenAiImageToOpenAiChat(_) => Ok(Vec::new()),
        }
    }
}


fn rewrite_model_directive_stream_line(
    report_context: &Value,
    line: Vec<u8>,
) -> Result<Vec<u8>, AiSurfaceFinalizeError> {
    let Some(display_model) = model_directive_display_model_from_report_context(report_context)
    else {
        return Ok(line);
    };
    let text = match std::str::from_utf8(&line) {
        Ok(text) => text,
        Err(_) => return Ok(line),
    };
    let trimmed_line_end = text.trim_end_matches(['\r', '\n']);
    let trailing = &text[trimmed_line_end.len()..];
    let Some((prefix, payload)) = trimmed_line_end.split_once(':') else {
        return Ok(line);
    };
    if prefix.trim() != "data" {
        return Ok(line);
    }
    let payload = payload.trim_start();
    if payload.is_empty() || payload == "[DONE]" {
        return Ok(line);
    }
    let mut value = match serde_json::from_str::<Value>(payload) {
        Ok(value) => value,
        Err(_) => return Ok(line),
    };
    if !rewrite_stream_payload_model(&mut value, &display_model) {
        return Ok(line);
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"data: ");
    output.extend(serde_json::to_vec(&value)?);
    output.extend_from_slice(trailing.as_bytes());
    Ok(output)
}

fn rewrite_openai_responses_compat_stream_line(
    report_context: &Value,
    line: Vec<u8>,
) -> Result<Vec<u8>, AiSurfaceFinalizeError> {
    let text = match std::str::from_utf8(&line) {
        Ok(text) => text,
        Err(_) => return Ok(line),
    };
    let trimmed_line_end = text.trim_end_matches(['\r', '\n']);
    let trailing = &text[trimmed_line_end.len()..];
    let Some((prefix, payload)) = trimmed_line_end.split_once(':') else {
        return Ok(line);
    };
    if prefix.trim() != "data" {
        return Ok(line);
    }
    let payload = payload.trim_start();
    if payload.is_empty() || payload == "[DONE]" {
        return Ok(line);
    }
    let mut value = match serde_json::from_str::<Value>(payload) {
        Ok(value) => value,
        Err(_) => return Ok(line),
    };
    let mut changed = rewrite_stream_payload_model_from_context(report_context, &mut value);
    let event_type = value
        .get("type")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if matches!(event_type, "response.completed" | "response.done") {
        if let Some(response) = value.get_mut("response").and_then(Value::as_object_mut) {
            changed |= ensure_modern_openai_responses_response_fields(response);
        }
    }
    if !changed {
        return Ok(line);
    }
    let mut output = Vec::new();
    output.extend_from_slice(b"data: ");
    output.extend(serde_json::to_vec(&value)?);
    output.extend_from_slice(trailing.as_bytes());
    Ok(output)
}

fn rewrite_stream_payload_model(value: &mut Value, display_model: &str) -> bool {
    let Some(object) = value.as_object_mut() else {
        return false;
    };
    let mut changed = false;
    for key in ["model", "modelVersion"] {
        if object.get(key).and_then(Value::as_str).is_some() {
            object.insert(key.to_string(), Value::String(display_model.to_string()));
            changed = true;
        }
    }
    for key in ["response", "message"] {
        if let Some(nested) = object.get_mut(key) {
            changed |= rewrite_stream_payload_model(nested, display_model);
        }
    }
    changed
}

fn rewrite_stream_payload_model_from_context(report_context: &Value, value: &mut Value) -> bool {
    let Some(display_model) = model_directive_display_model_from_report_context(report_context)
    else {
        return false;
    };
    rewrite_stream_payload_model(value, &display_model)
}


fn transform_standard_line(
    standard: &mut StreamingStandardFormatMatrix,
    report_context: &Value,
    line: Vec<u8>,
) -> Result<Vec<u8>, AiSurfaceFinalizeError> {
    standard.transform_line(report_context, line)
}

fn supports_standard_stream_rewrite(provider_api_format: &str, client_api_format: &str) -> bool {
    is_standard_provider_api_format(provider_api_format)
        && (is_standard_chat_client_api_format(client_api_format)
            || is_standard_cli_client_api_format(client_api_format))
}

/// Returns true for OpenAI Responses family formats that share the same SSE
/// wire format and can be passed through without parsing→rebuilding.
fn is_openai_responses_family(api_format: &str) -> bool {
    matches!(
        aether_ai_formats::normalize_api_format_alias(api_format).as_str(),
        "openai:responses" | "openai:responses:compact"
    )
}

/// Returns true when two API formats share the same SSE wire format and
/// can be passed through without parsing→rebuilding.  This covers:
///
/// - Exact matches after normalisation (e.g. `claude:messages` ↔ `claude:messages`)
/// - OpenAI Responses family (`openai:responses` ↔ `openai:responses:compact`)
fn is_same_format_family(provider_format: &str, client_format: &str) -> bool {
    let provider = aether_ai_formats::normalize_api_format_alias(provider_format);
    let client = aether_ai_formats::normalize_api_format_alias(client_format);
    if provider == client {
        return true;
    }
    // OpenAI Responses family shares the same wire format despite having
    // distinct format IDs.
    is_openai_responses_family(provider_format) && is_openai_responses_family(client_format)
}

fn is_standard_provider_api_format(api_format: &str) -> bool {
    matches!(
        aether_ai_formats::normalize_api_format_alias(api_format).as_str(),
        "openai:chat"
            | "openai:responses"
            | "openai:responses:compact"
            | "claude:messages"
            | "gemini:generate_content"
    )
}

fn is_standard_chat_client_api_format(api_format: &str) -> bool {
    matches!(
        api_format,
        "openai:chat" | "claude:messages" | "gemini:generate_content"
    )
}

fn is_standard_cli_client_api_format(api_format: &str) -> bool {
    matches!(
        aether_ai_formats::normalize_api_format_alias(api_format).as_str(),
        "openai:responses"
            | "openai:responses:compact"
            | "claude:messages"
            | "gemini:generate_content"
    )
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{
        maybe_build_ai_surface_stream_rewriter, resolve_finalize_stream_rewrite_mode,
        FinalizeStreamRewriteMode,
    };

    #[test]
    fn resolves_standard_mode_for_cross_format_standard_streams() {
        let report_context = json!({
            "provider_api_format": "claude:messages",
            "client_api_format": "openai:chat",
            "needs_conversion": true,
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::Standard)
        );
    }

    #[test]
    fn rejects_unsupported_non_conversion_streams() {
        let report_context = json!({
            "provider_api_format": "openai:chat",
            "client_api_format": "openai:chat",
            "needs_conversion": false,
        });
        assert_eq!(resolve_finalize_stream_rewrite_mode(&report_context), None);
    }

    #[test]
    fn resolves_model_directive_display_mode_for_same_format_standard_streams() {
        let report_context = json!({
            "provider_api_format": "openai:responses",
            "client_api_format": "openai:responses",
            "model": "gpt-5.5-xhigh",
            "mapped_model": "gpt-5.5",
            "needs_conversion": false,
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::ModelDirectiveDisplay)
        );
    }

    #[test]
    fn model_directive_display_rewriter_restores_response_model() {
        let report_context = json!({
            "provider_api_format": "openai:responses",
            "client_api_format": "openai:responses",
            "model": "gpt-5.5-xhigh",
            "mapped_model": "gpt-5.5",
            "needs_conversion": false,
        });
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"event: response.created\n\
data: {\"type\":\"response.created\",\"response\":{\"id\":\"resp_123\",\"object\":\"response\",\"model\":\"gpt-5.5\",\"status\":\"in_progress\"}}\n\n",
            )
            .expect("rewrite should succeed");
        let output = String::from_utf8(output).expect("output should be utf8");

        assert!(output.contains("event: response.created"));
        assert!(output.contains("\"model\":\"gpt-5.5-xhigh\""));
        assert!(!output.contains("\"model\":\"gpt-5.5\""));
    }

    #[test]
    fn standard_rewriter_converts_openai_responses_reasoning_delta_to_chat() {
        let report_context = json!({
            "provider_api_format": "openai:responses",
            "client_api_format": "openai:chat",
            "needs_conversion": true,
            "mapped_model": "gpt-5.4",
        });
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"event: response.reasoning_summary_text.delta\n\
data: {\"type\":\"response.reasoning_summary_text.delta\",\"response_id\":\"resp_reasoning_stream_123\",\"item_id\":\"rs_123\",\"output_index\":0,\"summary_index\":0,\"delta\":\"Need to inspect first.\"}\n\n",
            )
            .expect("rewrite should succeed");
        let output = String::from_utf8(output).expect("output should be utf8");

        assert!(output.contains("\"object\":\"chat.completion.chunk\""));
        assert!(output.contains("\"reasoning_content\":\"Need to inspect first.\""));
        assert!(!output.contains("\"content\""));
        assert!(!output.contains("data: [DONE]"));
    }

    #[test]
    fn standard_rewriter_converts_event_only_response_created_to_chat_role_chunk() {
        let report_context = json!({
            "provider_api_format": "openai:responses",
            "client_api_format": "openai:chat",
            "needs_conversion": true,
        });
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("rewriter should exist");
        let event_output = rewriter
            .push_chunk(b"event: response.created\n")
            .expect("event line should be buffered");
        assert!(event_output.is_empty());
        let output = rewriter
            .push_chunk(
                b"data: {\"response\":{\"id\":\"resp_created_123\",\"model\":\"gpt-5.4\",\"status\":\"in_progress\",\"output\":[]}}\n\n",
            )
            .expect("rewrite should succeed");
        let output = String::from_utf8(output).expect("output should be utf8");

        assert!(output.contains("\"object\":\"chat.completion.chunk\""));
        assert!(output.contains("\"id\":\"resp_created_123\""));
        assert!(output.contains("\"model\":\"gpt-5.4\""));
        assert!(output.contains("\"delta\":{\"role\":\"assistant\"}"));
    }

    #[test]
    fn explicit_responses_stream_format_preserves_function_call_metadata() {
        let report_context = json!({
            "provider_api_format": "openai:chat",
            "provider_stream_event_api_format": "openai:responses",
            "client_api_format": "openai:responses",
            "needs_conversion": true,
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::OpenAiResponsesCompat)
        );

        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("responses compat rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_chat_metadata_123\",\"call_id\":\"call_chat_metadata_123\",\"status\":\"completed\",\"arguments\":\"{}\",\"name\":\"lookup\",\"metadata\":{\"source\":\"chat\"},\"internal_chat_message_metadata_passthrough\":{\"turn_id\":\"turn_123\"}}}\n\n",
            )
            .expect("responses event should pass through");
        let output = String::from_utf8(output).expect("output should be utf8");

        assert!(output.contains("event: response.output_item.added"));
        assert!(output.contains("\"metadata\":{\"source\":\"chat\"}"));
        assert!(output
            .contains("\"internal_chat_message_metadata_passthrough\":{\"turn_id\":\"turn_123\"}"));
    }

    #[test]
    fn explicit_responses_stream_format_converts_to_chat_even_without_request_conversion() {
        let report_context = json!({
            "provider_api_format": "openai:chat",
            "provider_stream_event_api_format": "openai:responses",
            "client_api_format": "openai:chat",
            "needs_conversion": false,
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::Standard)
        );

        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("standard rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"event: response.output_item.added\ndata: {\"type\":\"response.output_item.added\",\"output_index\":0,\"item\":{\"type\":\"function_call\",\"id\":\"fc_chat_metadata_123\",\"call_id\":\"call_chat_metadata_123\",\"status\":\"completed\",\"arguments\":\"{}\",\"name\":\"lookup\",\"metadata\":{\"source\":\"chat\"},\"internal_chat_message_metadata_passthrough\":{\"turn_id\":\"turn_123\"}}}\n\n",
            )
            .expect("responses event should convert to chat");
        let output = String::from_utf8(output).expect("output should be utf8");

        assert!(output.contains("\"object\":\"chat.completion.chunk\""));
        assert!(output.contains("\"id\":\"call_chat_metadata_123\""));
        assert!(!output.contains("unsupported_stream_event"));
    }

    #[test]
    fn same_family_responses_passthrough_preserves_encrypted_content() {
        // When provider and client are both OpenAI Responses family,
        // the stream should pass through verbatim (only model name rewrite).
        // This preserves encrypted_content, original item IDs, etc.
        let report_context = json!({
            "provider_api_format": "openai:responses",
            "client_api_format": "openai:responses:compact",
            "needs_conversion": true,
            "model": "gpt-5.5-xhigh",
            "mapped_model": "gpt-5.5",
        });
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"event: response.output_item.added\n\
data: {\"type\":\"response.output_item.added\",\"response_id\":\"resp_123\",\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"rs_abc\",\"summary\":[],\"encrypted_content\":\"EWxvY2tlZENvbnRlbnQ=\"}}\n\n",
            )
            .expect("rewrite should succeed");
        let output = String::from_utf8(output).expect("output should be utf8");

        // Passthrough preserves the full payload structure
        assert!(output.contains("event: response.output_item.added"));
        assert!(output.contains("\"encrypted_content\":\"EWxvY2tlZENvbnRlbnQ=\""));
        assert!(output.contains("\"id\":\"rs_abc\""));
        assert!(output.contains("\"type\":\"reasoning\""));
    }

    #[test]
    fn same_family_responses_without_display_model_runs_terminal_compat_only() {
        let report_context = json!({
            "provider_api_format": "openai:responses",
            "client_api_format": "openai:responses:compact",
            "needs_conversion": true,
        });
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("responses compat rewriter should exist");
        let mut output = rewriter
            .push_chunk(
                b"event: response.output_item.added\n\
data: {\"type\":\"response.output_item.added\",\"response_id\":\"resp_123\",\"output_index\":0,\"item\":{\"type\":\"reasoning\",\"id\":\"rs_abc\",\"encrypted_content\":\"EWxvY2tlZA==\"}}\n\n",
            )
            .expect("non-terminal event should pass through");
        output.extend(
            rewriter
                .push_chunk(
                    b"event: response.completed\n\
data: {\"type\":\"response.completed\",\"response\":{\"id\":\"resp_123\",\"object\":\"response\",\"model\":\"gpt-5\",\"status\":\"completed\"}}\n\n",
                )
                .expect("terminal event should be normalized"),
        );
        let output = String::from_utf8(output).expect("output should be utf8");

        assert!(output.contains("\"encrypted_content\":\"EWxvY2tlZA==\""));
        assert!(output.contains("event: response.completed"));
        assert!(output.contains("\"output\":[]"));
        assert!(output.contains("\"output_text\":\"\""));
        assert!(output.contains("\"completed_at\":"));
    }

    #[test]
    fn same_format_claude_passthrough_with_display_model() {
        // Claude→Claude with needs_conversion=true should pass through
        // (only model name rewrite), not parse→rebuild.
        let report_context = json!({
            "provider_api_format": "claude:messages",
            "client_api_format": "claude:messages",
            "needs_conversion": true,
            "model": "claude-sonnet-4.5-high",
            "mapped_model": "claude-sonnet-4.5",
            "anthropic_compatibility_profile": "native_transparent",
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::ModelDirectiveDisplay)
        );
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"event: content_block_delta\n\
data: {\"type\":\"content_block_delta\",\"index\":1,\"future_event_field\":{\"keep\":true},\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Let me reason...\",\"future_delta_field\":42}}\n\n",
            )
            .expect("rewrite should succeed");
        let output = String::from_utf8(output).expect("output should be utf8");

        // Passthrough preserves the exact wire format
        assert!(output.contains("event: content_block_delta"));
        assert!(output.contains("\"thinking\":\"Let me reason...\""));
        assert!(output.contains("\"type\":\"thinking_delta\""));
        assert!(output.contains("\"future_event_field\":{\"keep\":true}"));
        assert!(output.contains("\"future_delta_field\":42"));
    }

    #[test]
    fn native_same_format_claude_without_display_model_is_verbatim() {
        let report_context = json!({
            "provider_api_format": "claude:messages",
            "client_api_format": "claude:messages",
            "needs_conversion": true,
        });
        assert_eq!(resolve_finalize_stream_rewrite_mode(&report_context), None);
        assert!(maybe_build_ai_surface_stream_rewriter(Some(&report_context)).is_none());
    }

    #[test]
    fn same_format_gemini_passthrough_with_display_model() {
        // Gemini→Gemini with needs_conversion=true should pass through.
        let report_context = json!({
            "provider_api_format": "gemini:generate_content",
            "client_api_format": "gemini:generate_content",
            "needs_conversion": true,
            "model": "gemini-2.5-pro-high",
            "mapped_model": "gemini-2.5-pro",
        });
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("rewriter should exist");
        let output = rewriter
            .push_chunk(
                b"data: {\"candidates\":[{\"content\":{\"parts\":[{\"text\":\"Hello\"}],\"role\":\"model\"}}],\"modelVersion\":\"gemini-2.5-pro\"}\n\n",
            )
            .expect("rewrite should succeed");
        let output = String::from_utf8(output).expect("output should be utf8");

        // Model version should be rewritten
        assert!(output.contains("\"modelVersion\":\"gemini-2.5-pro-high\""));
        assert!(!output.contains("\"modelVersion\":\"gemini-2.5-pro\""));
    }

    #[test]
    fn same_format_gemini_without_display_model_passes_through_verbatim() {
        // Gemini→Gemini without display model: no rewriter needed.
        let report_context = json!({
            "provider_api_format": "gemini:generate_content",
            "client_api_format": "gemini:generate_content",
            "needs_conversion": true,
        });
        assert!(maybe_build_ai_surface_stream_rewriter(Some(&report_context)).is_none());
    }

    #[test]
    fn resolves_openai_image_mode_for_same_format_image_streams() {
        let report_context = json!({
            "provider_api_format": "openai:image",
            "client_api_format": "openai:image",
            "needs_conversion": false,
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::OpenAiImage)
        );
    }

    #[test]
    fn rewrites_openai_image_stream_to_openai_chat_final_chunk() {
        let report_context = json!({
            "provider_api_format": "openai:image",
            "client_api_format": "openai:chat",
            "mapped_model": "gpt-image-2",
            "request_id": "trace-image-chat-stream",
            "needs_conversion": false,
        });
        assert_eq!(
            resolve_finalize_stream_rewrite_mode(&report_context),
            Some(FinalizeStreamRewriteMode::OpenAiImageToOpenAiChat)
        );
        let mut rewriter = maybe_build_ai_surface_stream_rewriter(Some(&report_context))
            .expect("image to chat stream rewriter should exist");

        let progress = rewriter
            .push_chunk(
                br#"event: response.image_generation_call.partial_image
data: {"type":"response.image_generation_call.partial_image","partial_image_b64":"cGFydGlhbA=="}

"#,
            )
            .expect("partial image should rewrite as progress");
        let progress_text = String::from_utf8(progress).expect("progress output should be utf8");
        assert!(progress_text.contains("\"object\":\"chat.completion.chunk\""));
        assert!(!progress_text.contains("cGFydGlhbA=="));

        let output_item = rewriter
            .push_chunk(
                br#"event: response.output_item.done
data: {"type":"response.output_item.done","item":{"type":"image_generation_call","id":"ig_1","result":"aGVsbG8=","output_format":"png"}}

"#,
            )
            .expect("output item should rewrite");
        let output_item_text = String::from_utf8(output_item).expect("output item should be utf8");
        assert!(output_item_text.is_empty());

        let final_output = rewriter
            .push_chunk(
                br#"event: response.completed
data: {"type":"response.completed","response":{"id":"resp_123","model":"gpt-image-2","tool_usage":{"image_gen":{"total_tokens":0}},"output":[]}}

"#,
            )
            .expect("completed event should rewrite");
        let final_text = String::from_utf8(final_output).expect("final output should be utf8");
        assert!(final_text.contains("\"object\":\"chat.completion.chunk\""));
        assert!(final_text.contains("![generated image](data:image/png;base64,aGVsbG8=)"));
        assert!(final_text.contains("data: [DONE]"));
        assert!(!final_text.contains("image_generation.completed"));
    }
}
