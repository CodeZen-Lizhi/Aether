# Responses Stream Heartbeats

## Contract

The Responses provider-event decoder in
`crates/aether-ai/formats/src/formats/openai/chat/stream.rs` treats `type=ping`
like the existing `keepalive` event: it emits no canonical frame and does not
start text, append content, report an unsupported event or complete the stream.
The same rule applies before the first content event and between content events.

Content and explicit completion retain their existing behavior. Do not ignore
all unknown event types to accommodate a heartbeat; unknown-event diagnostics
and provider errors still follow the existing conversion path.

## Regression Evidence

Use `StreamingStandardFormatMatrix::transform_line` to exercise the real
conversion route, not just the event classifier. The regression
`ignores_openai_responses_ping_without_interrupting_text_or_completion` in
`formats/shared/stream_core/format_matrix.rs` covers both Chat and Responses
clients, including the `openai:responses` provider-event format override:

1. A ping at the start emits nothing.
2. Text before and after another ping concatenates without loss or duplication.
3. There is no termination until `response.completed`.
4. Explicit completion terminates normally; a later `finish` emits nothing.

Run the `aether-ai-formats` library tests when changing this event classification
so existing error, unknown-event and completion conversions stay covered.
