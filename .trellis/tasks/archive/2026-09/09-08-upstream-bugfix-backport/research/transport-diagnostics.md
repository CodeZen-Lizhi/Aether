# Transport diagnostic URL protection (R2)

## Scope and evidence

- Baseline: `58c445ddc`; upstream reference: `36e9d21e3` (Display follow-up) and the relevant preceding formatter implementation. No complete application of `579f2c7cc`.
- At the baseline, `sanitize_upstream_url_text` removes query/fragment but leaves userinfo intact. Its parse-error fallback also leaves credentials in malformed authorities.
- At the baseline, `format_upstream_request_error` and `format_wreq_upstream_request_error` replace only the top-level metadata URL/URI. URLs in a different source message, or an error without URL metadata, bypass that replacement. `format_hyper_error_chain` returns every message unchanged.
- At the baseline, `ExecutionRuntimeTransportError` derives both `Debug` and `Error`. Dynamic strings are displayed directly; typed reqwest/wreq Debug recursively prints stored URL/URI and sources.
- Locked dependency sources confirm that reqwest `0.12.28` and wreq `6.0.0-rc.28` store a separate optional URL/URI and boxed error source. Their `Display` omits the source, while `Debug` includes it. Removing only top-level URL metadata cannot protect another URL carried in the source.
- The planner's `sanitize_upstream_url_for_log` has a different contract: HTTP(S)-only, invalid URLs replaced with a placeholder, and ownership in the planner/request path. The transport fix extends its existing helper instead of importing that higher-layer request redaction module.

## Implemented boundary

- Only diagnostic formatting in `apps/aether-gateway/src/execution_runtime/transport.rs` changes. Protect URL userinfo, query, and fragment while keeping ordinary causes, whitespace, Unicode text, status, kind, host, port, and path useful.
- Preserve the enum variants, typed underlying errors, source exposure, and stored message values. Do not modify captured bodies, response bytes, failure classification, candidate selection, timeouts, proxy options, or DNS policy.
- Avoid the upstream private-address masking, generic typed-error messages, global length truncation, and new privacy features. Those would discard diagnostics outside the authorized change.
- The main agent owns the separate SOCKS test module; this worker only adds its test module declaration.
- Extend `sanitize_upstream_url_text` to clear username/password and protect malformed/protocol-relative authorities. Reuse it from a private diagnostic text scanner in the same transport module. The scanner preserves surrounding text and uses paired diagnostic delimiters to distinguish independent URL entries. Bare comma/semicolon-separated URLs, including inside square brackets, can still be query/fragment values and are consumed together.
- All three existing error-chain formatters sanitize the final diagnostic, including sources without top-level URL metadata. Dynamic Display values use the same helper. Typed HTTP client variants use the existing rich formatters so the underlying cause is still visible.
- Replace derived Debug with explicit variant/field formatting. Status, phase, limit, and typed error ownership stay intact; no raw reqwest/wreq Debug source expansion crosses the formatting boundary.
- New regression tests live in `transport/diagnostic_tests.rs`. The existing `mod tests` lacked a `#[cfg(test)]` annotation even at `58c445ddc`; explicitly gate it alongside the two new test-only submodules. Its tests and production behavior are otherwise untouched.

Control-flow review: server response status is selected from enum variants/status fields; `SyncExecutionFailure::from_transport` selects fallback kinds from variants; `resolve_local_transport_failover_analysis_for_attempt` takes the attempt policy, not the formatted error message. None of these call sites or their classification/scheduling policies changed.

## Regression plan and status

Tests have been added before production changes in `transport::diagnostic_tests`:

1. URL userinfo removal, IPv6, malformed authority with Unicode path, and protocol-relative fallback.
2. Display and compact/pretty Debug across dynamic error variants.
3. Ordinary diagnostics, status, and stored error messages remain intact.
4. Multiple causes in a Hyper-style error chain.
5. Adjacent URLs embedded in Unicode diagnostics.
6. Real reqwest and wreq builder errors whose serialization source contains URLs, with and without primary URL/URI metadata; typed transport variants are checked too.
7. Real library HTTP status errors and JSON decode errors preserve status/reason and `kind=decode`.

All fixtures use synthetic credentials. Typed errors are constructed through local serialization or in-memory response conversion, without network requests.

The first compile attempt was blocked by the independently added SOCKS fixture accessing a private WebSocket module. The main agent fixed its test-only imports/re-exports before the diagnostic tests executed; that compile error is not counted as a failing regression.

Pre-fix execution of the original nine tests: **1 passed, 8 failed**. Failures explicitly exposed synthetic URL credentials in Display, derived Debug, the helper's userinfo handling, Hyper chains, reqwest/wreq sources with no metadata URL, adjacent Unicode diagnostics, and HTTP status error formatting. The test asserting preservation of ordinary diagnostics, stored message, and status passed.

The first implementation made all nine tests pass. The main agent then identified this additional concrete nested-query case:

```text
https://upstream-user:upstream-password@host.test/path?token=first-secret,second-secret&redirect=https://other.test/path?key=third-secret
```

A new targeted test reproduced leakage of `second-secret` from the first tokenizer's broad comma search (**0 passed, 1 failed**). The first correction required a comma/semicolon to immediately precede an adjacent URL; `redirect=URL` stayed inside the outer query and was removed with it. The adjacent URL list regression remained green at that stage. The bare-list splitting rule was subsequently removed by the final boundary correction below.

A further table-driven test covers JSON quotes, IPv6/wrapper brackets, parentheses in paths, malformed Unicode paths, protocol-relative proxy URLs, a Unicode prefix directly before the URL, punctuation inside query secrets, safe Unicode URL spelling, and a double-slash path before another URL. This implementation stage reached **11 passed, 0 failed**; the independent review below found further cases and supersedes that result.

```sh
cargo test -p aether-gateway --lib execution_runtime::transport::diagnostic_tests -- --nocapture
```

The exact nested-query red-test command was:

```sh
cargo test -p aether-gateway --lib execution_runtime::transport::diagnostic_tests::nested_url_in_query_does_not_expose_values_before_it -- --exact --nocapture
```

Additional checks:

| Command | Result |
| --- | --- |
| `cargo test -p aether-gateway --lib execution_runtime::transport::tests -- --nocapture` | 31 passed, 0 failed, 1 pre-existing ignored H2C on-wire timing/environment test |
| `rustfmt --edition 2021 --check apps/aether-gateway/src/execution_runtime/transport.rs` | Passed, including the test submodules |
| `git diff --check` | Passed |

The gateway test build reported the existing `unreachable pattern` warning in `dispatch/refs.rs:71`; this worker did not change that unrelated code. The main agent completed the package checks and strict Clippy baseline comparison; final results are recorded in `verification.md`.

## Independent review corrections

The independent checker reproduced three remaining scanner problems. The worker added exact regressions before modifying production code again:

1. **Apostrophes in URLs**: `https://user:pa'ss-secret@host.test/path?key=secret` was copied through unchanged because the apostrophe terminated the URL before its userinfo separator. Query/fragment values such as `socks5h://host.test/path?key=first'second-secret` also leaked their suffixes.
2. **Paired diagnostic groups**: `error at (https://user:pass@host.test/path?key=secret);cause=(TLS)` lost the entire `;cause=(TLS)` suffix because the previous scanner selected the last closing parenthesis. A nested redirect URL followed by a separate `,source=(URL)` similarly lost the source diagnostic.
3. **URL-valued secrets**: `url=https://host.test/path?token=opaque,https://private-value.test/super-secret` exposed the nested URL's host/path by mistaking it for a separate diagnostic URL. Semicolon and fragment variants had the same defect.

The first two regressions failed in the full diagnostic run (**11 passed, 2 failed**). The third targeted regression also failed (**0 passed, 1 failed**):

```sh
cargo test -p aether-gateway --lib execution_runtime::transport::diagnostic_tests::url_valued_query_and_fragment_secrets_are_not_treated_as_url_lists -- --exact --nocapture
```

The first independent-review correction was confined to the same formatting boundary:

- Skip grouping/quote punctuation inside userinfo until its final `@`; apostrophes elsewhere are URL characters unless an enclosing diagnostic quote and following diagnostic syntax identify the closing quote.
- Match paired local parentheses/brackets, including IPv6 and path/query parentheses, against the corresponding surrounding diagnostic group. Stop before the following `;cause=(TLS)` or `,source=(URL)` instead of searching for the last closer in the whole token.
- Split adjacent URLs only when a square bracket immediately introduced a URL list. Keep URL-valued query/fragment contents together in ordinary URLs and bracketed scalar metadata.
- Consume nested URL matches that belong to the current URL, but retain a later independent list/source match. This is checked with a nested redirect in the first list entry and a distinct second URL.

At that stage, tests included 9 apostrophe scenarios (userinfo/query/fragment, enclosing quotes, punctuation before `@`, and a subsequent quoted source), 8 grouped/context scenarios (paired and unpaired URL punctuation, IPv6, nested source/list URLs, and scalar metadata), and comma/semicolon/fragment URL-valued secrets. The original diagnostic preservation cases remained intact.

Verification after that correction stage:

| Command | Result |
| --- | --- |
| `cargo test -p aether-gateway --lib execution_runtime::transport::diagnostic_tests -- --nocapture` | **14 passed, 0 failed** |
| `rustfmt --edition 2021 --check apps/aether-gateway/src/execution_runtime/transport.rs` | Passed |
| `git diff --check` | Passed |

Cargo ownership was returned to the main agent after this run, then reassigned for the bounded follow-up below. No SOCKS, scheduling, original response, or configuration code was modified during these corrections.

## Final boundary correction

The independent checker used a harness extracted from the actual helper to reproduce three remaining cases before this correction:

1. `url='socks5h://host.test/path?key=first';token=second-secret'` and `url=(https://host.test/path?key=first);token=second-secret)` leaked the synthetic token after a premature closing quote/group. These are now regression cases; a candidate boundary is rejected when its suffix leaves an unmatched quote/group.
2. `urls=[https://host.test/path?token=opaque,https://private-value.test/super-secret]` exposed the nested URL. The main agent approved removing all bare-list comma/semicolon splitting. Query/fragment contents are consumed together; the independent-URL preservation fixture now uses separately paired entries such as `urls=[(URL1),(URL2)]`.
3. `url=(https://host.test);peer=user@example.test` and the quoted equivalent let authority inspection cross the candidate closing delimiter and mistake a subsequent peer field for URL userinfo. Userinfo lookup is now bounded to the candidate URL body, retaining both the original host and the separate peer diagnostic.

Suffix balancing preserves the explicitly required controls: `;cause=(TLS)`, `;cause='TLS'`, nested `((URL))`, and a separate quoted source with apostrophe-bearing userinfo (`;source='https://user:pa'ss-secret@proxy.test/path?key=secret'`). The suffix scanner skips URL userinfo when balancing quotes, so it does not mistake the source password's apostrophe for a closing diagnostic quote. The older bare-list splitting rule above is historical and no longer present.

Final targeted verification:

| Command | Result |
| --- | --- |
| `cargo test -p aether-gateway --lib execution_runtime::transport::diagnostic_tests -- --nocapture` | **14 passed, 0 failed**, including the final boundary cases and preservation controls |
| `rustfmt --edition 2021 --check apps/aether-gateway/src/execution_runtime/transport.rs` | Passed |
| `git diff --check` | Passed |

The last source-only cleanup removes an unnecessary `mut` from the consumed regex iterator; behavior is unchanged. The only warning in the targeted Cargo run remains the pre-existing unreachable pattern in `dispatch/refs.rs:71`. The prior 31-pass transport-module result predates these scanner refinements; the final package run included the transport tests with no transport failures. The main agent completed the package-wide checks and baseline comparison, including the unchanged quality diagnostics and the separately recorded Gemini test fluctuation; see `verification.md`.

## Independently owned SOCKS fixture result

The requested additional run was:

```sh
cargo test -p aether-gateway --lib execution_runtime::transport::proxy_dns_tests -- --nocapture
```

Initial result: **3 passed, 1 failed**. Browser HTTP, ordinary WebSocket, and browser WebSocket passed. The ordinary HTTP fixture rejected `Domain("[::1]")` as an unexpected `socks5` DNS mode at `proxy_dns_tests.rs:152`. The main agent verified that the locked dependency encodes an already resolved numeric IPv6 address this way and adjusted the assertion while still rejecting the original hostname. The final result is **4 passed, 0 failed**, covering 8 configurations. No transport/proxy production behavior was changed to satisfy this fixture. Details and interoperability limits are recorded in `proxy-and-tunnel.md`.

## Remaining boundary

This change protects URL-shaped diagnostics at the existing transport formatting boundaries. It deliberately retains stored error objects/messages and does not scan arbitrary credentials in prose, request/response bodies, or persisted captures. Independent review is complete with no unresolved in-scope findings; package-wide results and baseline limitations are recorded in `review.md` and `verification.md`.

## Dependency references

- Context7 lookup: `/seanmonstar/reqwest`, error URL/source handling documentation.
- Authoritative version-specific implementation: Cargo registry `reqwest-0.12.28/src/error.rs`, `wreq-6.0.0-rc.28/src/error.rs`, and the request/response constructors used by the regression fixtures.
