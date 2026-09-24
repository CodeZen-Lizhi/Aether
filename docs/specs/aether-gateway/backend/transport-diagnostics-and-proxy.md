# Transport Diagnostics and SOCKS DNS

## Scope / Trigger

These contracts apply when formatting upstream transport errors or changing the
HTTP/WebSocket proxy builders. Personal-branch proxy configuration and raw
upstream response capture must retain their existing meaning.

## Signatures

The diagnostic boundary lives in `apps/aether-gateway/src/execution_runtime/transport.rs`:

```rust
fn format_upstream_request_error(err: &reqwest::Error) -> String;
fn format_wreq_upstream_request_error(err: &wreq::Error) -> String;
fn format_hyper_error_chain(err: &dyn std::error::Error) -> String;
```

`ExecutionRuntimeTransportError` also owns its `Display` and `Debug` output.
HTTP clients consume `ProxySnapshot.url`; WebSocket connections use
`handlers/proxy/websocket/transport.rs::connect_upstream_websocket`.

## Contracts

- Remove URL userinfo, query and fragment from diagnostic output, including URLs
  embedded in dynamic messages and every formatted source in an error chain.
  A source can contain URLs even when the outer reqwest error has no `url()` or
  wreq error has no `uri()`.
- Preserve scheme, host, port, path, HTTP status, error kind and ordinary causes.
  Preserve surrounding Unicode text and context after each URL. Malformed URLs
  must not cause a panic or expose userinfo through the parsing fallback.
- A query/fragment value may itself contain a URL, comma, semicolon or apostrophe.
  Do not treat those characters alone as an external diagnostic boundary,
  even inside a bare `[...]` list.
  Keep ambiguous nested values inside the outer URL's redaction; when testing
  distinct adjacent URLs, use explicit quotes, individual groups or whitespace.
  Retain later balanced `cause=(...)` and `source='...'` diagnostic fields.
- Formatting must not mutate stored messages, typed errors, response bodies,
  request bodies or capture policy. Retry, circuit and timeout behavior is out
  of scope for diagnostic formatting.
- `socks5://` resolves the target locally and sends a numeric address to the SOCKS server.
  `socks5h://` sends the target hostname to the SOCKS server for remote resolution.
  This applies to ordinary and browser-profile HTTP and WebSocket clients.
- Do not rewrite one SOCKS scheme to the other. Both are intentional supported
  configurations; a different upstream DNS policy is not evidence of a local bug.

## Validation and Error Matrix

| Input | Required result |
| --- | --- |
| `https://user:secret@host.test:8443/path?key=secret#secret` | `https://host.test:8443/path` |
| Several credential URLs in one error/source chain | Protect every URL, retaining intervening causes |
| `503 Service Unavailable`, decode kind, TLS/socket cause | Keep the status, classification and cause readable |
| Ordinary text without a credential URL | Keep its content and formatting |
| `socks5://` with a localhost target | SOCKS CONNECT uses a resolved loopback address, never the original hostname |
| `socks5h://` with a `.invalid` target | SOCKS CONNECT uses the original hostname; no local target lookup |

## Good / Base / Bad Cases

- Good: a TLS failure shows the upstream host/path and proxy host without their credentials.
- Base: a normal timeout remains understandable and keeps its existing error kind.
- Bad: deriving `Debug` over a URL-bearing source exposes its raw fields; replacing
  the whole diagnostic with a generic label loses the cause needed for debugging.

## Required Tests

- `transport/diagnostic_tests.rs`: dynamic variants, `Display`, ordinary and
  alternate `Debug`, typed sources with and without a primary URL/URI, multiple
  URLs, nested query values, legitimate apostrophes, paired diagnostic groups,
  Unicode, invalid ports, IPv6, status/kind and unchanged stored messages.
- `transport/proxy_dns_tests.rs`: real client builders and WebSocket handshake
  entry points against a loopback SOCKS fixture. Assert CONNECT address semantics,
  port, HTTP Host/path and successful HTTP/WS responses for both schemes and both
  client profiles. Never use a real provider or production proxy credential.

With the locked reqwest 0.12.28/hyper-util 0.1.20, a locally resolved IPv6 address
can use SOCKS domain encoding such as `[::1]`. The DNS assertion accepts only a
parseable numeric loopback address in that case, never `localhost`. This verifies
where DNS runs; it does not certify every external proxy's IPv6 interoperability.

## Wrong vs Correct

Wrong: redact only `err.url()` or overwrite every proxy URL with `socks5h://`.

Correct: sanitize the complete formatted diagnostic, preserving non-sensitive
causes, and pass the user's selected SOCKS scheme through unchanged.
