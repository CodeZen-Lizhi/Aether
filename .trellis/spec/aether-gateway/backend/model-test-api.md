# Provider Model Test API Contract

`/api/admin/provider-query/test-model` and `/api/admin/provider-query/test-model-failover`
are a **diagnostic channel**, not a serving path. They exist so an operator can verify a
provider/key actually works **before** enabling it for traffic.

## 1. Scope / Trigger

- Trigger: admin UI "模型测试" button in the provider detail drawer.
- Behavior contract introduced by task `model-test-bypass-status` (2026-09-03).

## 2. Signatures

- `POST /api/admin/provider-query/test-model`
- `POST /api/admin/provider-query/test-model-failover`
- Payload: `{ provider_id, model?, failover_models?, api_format?, endpoint_id?,
  api_key_id? / api_key_ids?, apply_model_mapping?, mapped_model_name?, message?,
  request_body?, request_headers? }` (see `handlers/admin/provider/query/models/model_test.rs`).

## 3. Contracts

- **Runtime on/off state is never a precondition.** `provider.is_active`,
  `endpoint.is_active`, and `key.is_active` are ignored by candidate building and
  execution. Health/circuit state only affects candidate ordering
  (`provider_query_test_key_sort_key`), never eligibility.
- **Key existence is the only hard precondition**: the provider must have at least one
  key that passes the format-compatibility check
  (`provider_query_key_supports_endpoint`). If none exists the request fails without
  any upstream call: `404 {"detail": "No usable API key found for this provider"}`
  (`ADMIN_PROVIDER_QUERY_NO_ACTIVE_TEST_CANDIDATE_DETAIL`).
- **Endpoint fallback**: active endpoints are preferred; if none is active, any
  endpoint (including disabled) is used so the test stays possible.
- **Non-status validations stay in place**: `selected_key_ids` must exist,
  `allowed_models` still demotes disallowed keys to `key_model_not_allowed` skipped
  candidates, and real capability limits (api-format mismatch, header/body rules,
  oauth resolution, proxy/profile) still produce skipped attempts.
- **Upstream stream policy**: the test executes non-streaming plans. Provider bodies
  for non-streaming tests must omit the `stream` field rather than send
  `stream: false` — `enforce_request_body_stream_field` only rewrites `stream` when
  the request carries the field, so default test bodies must not include it.

## 4. Validation & Error Matrix

| Condition | Result |
|-----------|--------|
| Provider id missing/unknown | 400 `provider_id is required` / 404 `Provider not found` |
| Model missing | 400 `model is required` |
| `api_key_ids` names a nonexistent key | 404 `API Key not found` |
| No format-compatible key (incl. zero keys) | 404 `No usable API key found for this provider` |
| Key disabled, provider disabled, endpoint disabled | **Test still executes** |
| Key blocked by `allowed_models` | Attempt recorded as `skipped` with `key_model_not_allowed` |
| Transport capability unsupported (format/rules/oauth/proxy) | Attempt `skipped` with the capability reason |

## 5. Good/Base/Bad Cases

- Good: provider disabled, one disabled key → upstream called, `success` reflects the
  real response.
- Base: provider active, keys active → unchanged behavior.
- Bad: provider has no keys → 404, zero upstream calls.

## 6. Tests Required

- `gateway_handles_test_model_with_inactive_provider_endpoint_and_key` —
  provider/endpoint/key all `is_active=false` must still execute and succeed.
- `gateway_rejects_test_model_when_provider_has_no_keys` — 404 + exact detail string,
  and the execution runtime must not be called (panic guard).
- `provider_query_capability_probe_ignores_inactive_status` /
  `provider_query_grok_test_reason_ignores_inactive_status` — unit contract for the
  probe below.
- `gateway_handles_openai_responses_test_model_locally` — non-streaming responses
  body must not carry `stream: false`.
  All live in `src/handlers/admin/provider/query/models/model_test/tests.rs` and
  `src/tests/control/admin/provider_query.rs`.

## 7. Wrong vs Correct

#### Wrong

Weakening `crates/aether-provider/transport/src/policy.rs` (`*_unsupported_reason`
functions) to skip `is_active` checks — those same functions are reused by the live
proxy path (`ai_serving/planner`), where disabled providers/keys must stay rejected.

#### Correct

In the test path only, neutralize state on a clone before calling the shared policy
checks (`provider_query_test_transport_for_capability_probe`):

```rust
fn provider_query_test_transport_for_capability_probe(
    transport: &AdminGatewayProviderTransportSnapshot,
) -> AdminGatewayProviderTransportSnapshot {
    let mut transport = transport.clone();
    transport.provider.is_active = true;
    transport.endpoint.is_active = true;
    transport.key.is_active = true;
    transport
}
```

The live-path semantics in `aether-provider-transport` must never change for this
feature.
