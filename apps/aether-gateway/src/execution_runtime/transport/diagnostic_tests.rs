use std::error::Error as _;

use super::{
    format_hyper_error_chain, format_upstream_request_error, format_wreq_upstream_request_error,
    sanitize_upstream_request_error_detail, sanitize_upstream_url_text,
    ExecutionRuntimeTransportError,
};

const UPSTREAM_URL: &str = "https://upstream-user:upstream-password@127.0.0.1:8443/v1/responses?key=query-secret#fragment-secret";
const PROXY_URL: &str = "socks5h://proxy-user:proxy-password@proxy.example.test:1080/connect?auth=proxy-query-secret#proxy-fragment-secret";

fn diagnostic_detail() -> String {
    format!("certificate expired at ({UPSTREAM_URL}); proxy={PROXY_URL}; retries exhausted")
}

fn assert_url_secrets_removed(detail: &str) {
    for secret in [
        "upstream-user",
        "upstream-password",
        "query-secret",
        "fragment-secret",
        "proxy-user",
        "proxy-password",
    ] {
        assert!(
            !detail.contains(secret),
            "URL secret {secret:?} leaked: {detail}"
        );
    }
}

fn assert_diagnostic_preserved(detail: &str) {
    assert_url_secrets_removed(detail);
    for expected in [
        "certificate expired",
        "127.0.0.1:8443/v1/responses",
        "proxy.example.test:1080/connect",
        "retries exhausted",
    ] {
        assert!(detail.contains(expected), "missing {expected:?}: {detail}");
    }
}

fn dynamic_errors() -> Vec<ExecutionRuntimeTransportError> {
    let detail = diagnostic_detail();
    vec![
        ExecutionRuntimeTransportError::UnsupportedContentEncoding(detail.clone()),
        ExecutionRuntimeTransportError::InvalidHeaderName(detail.clone()),
        ExecutionRuntimeTransportError::InvalidHeaderValue(detail.clone()),
        ExecutionRuntimeTransportError::UnsupportedTransportProfile(detail.clone()),
        ExecutionRuntimeTransportError::BrowserBody(detail.clone()),
        ExecutionRuntimeTransportError::UpstreamHttpStatus {
            status_code: 503,
            message: format!("503 Service Unavailable: {detail}"),
        },
        ExecutionRuntimeTransportError::UpstreamRequest(detail.clone()),
        ExecutionRuntimeTransportError::UpstreamResponseDecode {
            encoding: "gzip".to_string(),
            message: detail.clone(),
        },
        ExecutionRuntimeTransportError::UpstreamResponseDecode {
            encoding: detail.clone(),
            message: "invalid compression header".to_string(),
        },
        ExecutionRuntimeTransportError::RelayError(detail.clone()),
        ExecutionRuntimeTransportError::BodyEncode(serde::ser::Error::custom(&detail)),
        ExecutionRuntimeTransportError::InvalidJson(serde::de::Error::custom(&detail)),
    ]
}

#[test]
fn upstream_url_sanitization_removes_userinfo_and_preserves_address() {
    let detail = format!("request to ({UPSTREAM_URL}); source repeated {UPSTREAM_URL}");
    let (detail, url) = sanitize_upstream_request_error_detail(&detail, UPSTREAM_URL);
    assert_eq!(url, "https://127.0.0.1:8443/v1/responses");
    assert_eq!(
        detail,
        "request to (https://127.0.0.1:8443/v1/responses); source repeated https://127.0.0.1:8443/v1/responses"
    );

    for (raw, expected) in [
        (
            "https://upstream-user:upstream-password@[::1]:8443/path?key=query-secret#fragment-secret",
            "https://[::1]:8443/path",
        ),
        (
            "https://upstream-user:upstream-password@host.invalid:bad/路径?key=query-secret#fragment-secret",
            "https://host.invalid:bad/路径",
        ),
        (
            "//upstream-user:upstream-password@proxy.example.test/path?key=query-secret#fragment-secret",
            "//proxy.example.test/path",
        ),
        ("/v1/responses?key=query-secret#fragment-secret", "/v1/responses"),
    ] {
        assert_eq!(sanitize_upstream_url_text(raw), expected);
    }
}

#[test]
fn transport_error_display_removes_url_secrets() {
    for error in dynamic_errors() {
        assert_diagnostic_preserved(&error.to_string());
    }
}

#[test]
fn transport_error_debug_removes_url_secrets() {
    for error in dynamic_errors() {
        assert_diagnostic_preserved(&format!("{error:?}"));
        assert_diagnostic_preserved(&format!("{error:#?}"));
    }
}

#[test]
fn transport_error_preserves_plain_diagnostics_status_and_stored_message() {
    let plain = "TLS certificate expired: issuer=test\n  socket error (os error 54); retry=2 [kind=connect]，原因仍可见？";
    assert_eq!(
        ExecutionRuntimeTransportError::RelayError(plain.to_string()).to_string(),
        format!("hub relay request failed: {plain}")
    );
    assert_eq!(
        format_hyper_error_chain(&std::io::Error::other(plain)),
        plain
    );

    let message = format!("503 Service Unavailable: {}", diagnostic_detail());
    let error = ExecutionRuntimeTransportError::UpstreamHttpStatus {
        status_code: 503,
        message: message.clone(),
    };
    assert!(error.to_string().contains("503 Service Unavailable"));
    assert!(format!("{error:?}").contains("status_code: 503"));
    let ExecutionRuntimeTransportError::UpstreamHttpStatus {
        status_code,
        message: stored,
    } = error
    else {
        unreachable!();
    };
    assert_eq!(status_code, 503);
    assert_eq!(
        stored, message,
        "formatting must not mutate the stored upstream error"
    );
}

#[derive(Debug, thiserror::Error)]
#[error("{message}")]
struct ChainedError {
    message: String,
    source: std::io::Error,
}

#[test]
fn hyper_error_chain_removes_urls_in_every_cause_and_preserves_context() {
    let error = ChainedError {
        message: format!("TLS handshake at ({UPSTREAM_URL})"),
        source: std::io::Error::other(format!(
            "certificate expired; proxy=<{PROXY_URL}>; retries exhausted"
        )),
    };
    assert!(error.source().is_some());
    let detail = format_hyper_error_chain(&error);
    assert_diagnostic_preserved(&detail);
    assert_eq!(
        detail,
        "TLS handshake at (https://127.0.0.1:8443/v1/responses): certificate expired; proxy=<socks5h://proxy.example.test:1080/connect>; retries exhausted"
    );
}

#[test]
fn transport_error_preserves_unicode_and_adjacent_url_diagnostics() {
    let error = ExecutionRuntimeTransportError::UpstreamRequest(format!(
        "连接失败\n  urls=[({UPSTREAM_URL}),({PROXY_URL})];原因=证书过期"
    ));
    let detail = error.to_string();
    assert_url_secrets_removed(&detail);
    assert_eq!(
        detail,
        "failed to execute upstream request: 连接失败\n  urls=[(https://127.0.0.1:8443/v1/responses),(socks5h://proxy.example.test:1080/connect)];原因=证书过期"
    );
}

#[test]
fn nested_url_in_query_does_not_expose_values_before_it() {
    let message = "proxy=https://upstream-user:upstream-password@host.test/path?token=first-secret,second-secret&redirect=https://other.test/path?key=third-secret";
    let error = ExecutionRuntimeTransportError::UpstreamRequest(message.to_string());
    let display = error.to_string();
    let debug = format!("{error:?}");
    for detail in [&display, &debug] {
        assert_url_secrets_removed(detail);
        for secret in ["first-secret", "second-secret", "third-secret"] {
            assert!(!detail.contains(secret), "query secret leaked: {detail}");
        }
    }
    assert_eq!(
        display,
        "failed to execute upstream request: proxy=https://host.test/path"
    );
}

#[test]
fn url_valued_query_and_fragment_secrets_are_not_treated_as_url_lists() {
    for secret_suffix in [
        "?token=opaque,https://private-value.test/super-secret",
        "?token=opaque;https://private-value.test/super-secret",
        "#fragment-secret,https://private-value.test/super-secret",
    ] {
        let error = ExecutionRuntimeTransportError::UpstreamRequest(format!(
            "url=https://host.test/path{secret_suffix}"
        ));
        let display = error.to_string();
        assert!(
            !display.contains("secret") && !display.contains("private-value"),
            "URL-valued secret leaked: {display}"
        );
        assert_eq!(
            display,
            "failed to execute upstream request: url=https://host.test/path"
        );
        assert!(!format!("{error:?}").contains("private-value"));

        let error = ExecutionRuntimeTransportError::UpstreamRequest(format!(
            "urls=[https://host.test/path{secret_suffix}]"
        ));
        assert_eq!(
            error.to_string(),
            "failed to execute upstream request: urls=[https://host.test/path]"
        );
        assert!(!format!("{error:?}").contains("private-value"));
    }
}

#[test]
fn transport_error_preserves_url_punctuation_and_safe_unicode_text() {
    for (message, expected) in [
        (
            r#"{"url":"https://upstream-user:upstream-password@host.test/path?key=query-secret#fragment-secret","cause":"connection reset"}"#,
            r#"{"url":"https://host.test/path","cause":"connection reset"}"#,
        ),
        (
            "addresses=[https://upstream-user:upstream-password@[::1]:8443/path?key=query-secret#fragment-secret]; cause=reset",
            "addresses=[https://[::1]:8443/path]; cause=reset",
        ),
        (
            "failed at (https://upstream-user:upstream-password@host.test/(beta)/path?key=query-secret#fragment-secret); cause=reset",
            "failed at (https://host.test/(beta)/path); cause=reset",
        ),
        (
            "bad=HTTPS://upstream-user:upstream-password@host.test:invalid/路径?key=query-secret#fragment-secret; 原因=端口无效",
            "bad=HTTPS://host.test:invalid/路径; 原因=端口无效",
        ),
        (
            "proxy=(//proxy-user:proxy-password@proxy.test:1080/path?key=query-secret#fragment-secret)",
            "proxy=(//proxy.test:1080/path)",
        ),
        (
            "连接失败https://upstream-user:upstream-password@host.test/path?key=query-secret#fragment-secret",
            "连接失败https://host.test/path",
        ),
        (
            "url=(https://host.test/path?key=query-secret,second-secret;third-secret#fragment-secret); cause=reset",
            "url=(https://host.test/path); cause=reset",
        ),
        (
            "retry at https://EXAMPLE.test:443/路径 -> connection reset",
            "retry at https://EXAMPLE.test:443/路径 -> connection reset",
        ),
        (
            "urls=[(https://upstream-user:upstream-password@host.test/double//path?key=query-secret#fragment-secret),(https://proxy-user:proxy-password@proxy.test/path?key=query-secret#fragment-secret)]",
            "urls=[(https://host.test/double//path),(https://proxy.test/path)]",
        ),
    ] {
        let error = ExecutionRuntimeTransportError::RelayError(message.to_string());
        let display = error.to_string();
        assert_url_secrets_removed(&display);
        assert_eq!(display, format!("hub relay request failed: {expected}"));
        assert_url_secrets_removed(&format!("{error:#?}"));
    }
}

#[test]
fn transport_error_removes_url_secrets_containing_apostrophes() {
    for (message, expected) in [
        (
            "error at https://user:pa'ss-secret@host.test/path?key=secret",
            "error at https://host.test/path",
        ),
        (
            "error at socks5h://host.test/path?key=first'second-secret",
            "error at socks5h://host.test/path",
        ),
        (
            "error at socks5h://host.test/path#first'second-secret",
            "error at socks5h://host.test/path",
        ),
        (
            "error at 'https://user:pa'ss-secret@host.test/path?key=secret'; cause=reset",
            "error at 'https://host.test/path'; cause=reset",
        ),
        (
            "error at 'socks5h://host.test/path?key=first'second-secret'; cause=reset",
            "error at 'socks5h://host.test/path'; cause=reset",
        ),
        (
            "error at 'socks5h://host.test/path#first'second-secret';cause=(TLS)",
            "error at 'socks5h://host.test/path';cause=(TLS)",
        ),
        (
            "error at 'socks5h://host.test/path?key=first';second-secret';cause=(TLS)",
            "error at 'socks5h://host.test/path';cause=(TLS)",
        ),
        (
            "error at 'https://user:pa';ss-secret@host.test/path?key=secret';cause=(TLS)",
            "error at 'https://host.test/path';cause=(TLS)",
        ),
        (
            "url='socks5h://host.test/path?key=first'secret',source='https://proxy:password@proxy.test/path?key=second-secret'",
            "url='socks5h://host.test/path',source='https://proxy.test/path'",
        ),
        (
            "url='socks5h://host.test/path?key=first';token=second-secret'",
            "url='socks5h://host.test/path'",
        ),
        (
            "url='socks5h://host.test/path?key=first';cause='TLS'",
            "url='socks5h://host.test/path';cause='TLS'",
        ),
        (
            "url='socks5h://host.test/path?key=first';source='https://user:pa'ss-secret@proxy.test/path?key=secret'",
            "url='socks5h://host.test/path';source='https://proxy.test/path'",
        ),
    ] {
        let error = ExecutionRuntimeTransportError::UpstreamRequest(message.to_string());
        let display = error.to_string();
        assert!(!display.contains("secret"), "URL secret leaked: {display}");
        assert_eq!(
            display,
            format!("failed to execute upstream request: {expected}")
        );
        assert!(!format!("{error:?}").contains("secret"));
    }
}

#[test]
fn transport_error_preserves_context_after_wrapped_urls() {
    for (message, expected) in [
        (
            "error at (https://user:pass@host.test/path?key=secret);cause=(TLS)",
            "error at (https://host.test/path);cause=(TLS)",
        ),
        (
            "url=(https://user:pass@host.test/path?redirect=https://nested.test/a),source=(https://proxy:password@proxy.test/path?key=proxy-secret)",
            "url=(https://host.test/path),source=(https://proxy.test/path)",
        ),
        (
            "url=(https://user:pass@host.test/(beta)/path?key=first(second)secret);cause=(TLS)",
            "url=(https://host.test/(beta)/path);cause=(TLS)",
        ),
        (
            "urls=[https://user:pass@[::1]:8443/(beta)/path?key=secret];cause=[TLS]",
            "urls=[https://[::1]:8443/(beta)/path];cause=[TLS]",
        ),
        (
            "error at (https://user:pa)ss-secret@host.test/path?key=secret);cause=(TLS)",
            "error at (https://host.test/path);cause=(TLS)",
        ),
        (
            "error at (socks5h://host.test/path?key=first)second-secret);cause=(TLS)",
            "error at (socks5h://host.test/path);cause=(TLS)",
        ),
        (
            "urls=[(https://user:pass@host.test/path?redirect=https://private-value.test/inner-secret),(https://proxy:password@proxy.test/path?key=proxy-secret)]",
            "urls=[(https://host.test/path),(https://proxy.test/path)]",
        ),
        (
            "[url=https://host.test/path?token=opaque,https://private-value.test/super-secret]",
            "[url=https://host.test/path]",
        ),
        (
            "url=(https://host.test/path?key=first);token=second-secret)",
            "url=(https://host.test/path)",
        ),
        (
            "url=((https://host.test/path?key=secret));cause=(TLS)",
            "url=((https://host.test/path));cause=(TLS)",
        ),
        (
            "url=(https://host.test);peer=user@example.test",
            "url=(https://host.test);peer=user@example.test",
        ),
        (
            "url='https://host.test';peer=user@example.test",
            "url='https://host.test';peer=user@example.test",
        ),
    ] {
        let error = ExecutionRuntimeTransportError::RelayError(message.to_string());
        let display = error.to_string();
        assert!(!display.contains("secret"), "URL secret leaked: {display}");
        assert_eq!(display, format!("hub relay request failed: {expected}"));
        assert!(!format!("{error:#?}").contains("secret"));
    }
}

struct SerializationFailure;

impl serde::Serialize for SerializationFailure {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Err(serde::ser::Error::custom(diagnostic_detail()))
    }
}

fn reqwest_source_error() -> reqwest::Error {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("test HTTP client")
        .post("http://unused.invalid/")
        .json(&SerializationFailure)
        .build()
        .expect_err("serialization must fail before any network request")
}

fn wreq_source_error() -> wreq::Error {
    wreq::Client::builder()
        .no_proxy()
        .build()
        .expect("test browser HTTP client")
        .post("http://unused.invalid/")
        .json(&SerializationFailure)
        .build()
        .expect_err("serialization must fail before any network request")
}

#[test]
fn reqwest_formatter_protects_typed_sources_with_and_without_a_primary_url() {
    let error = reqwest_source_error();
    assert!(error.source().is_some());
    assert!(error.url().is_none());
    assert_diagnostic_preserved(&format_upstream_request_error(&error));

    let error = error.with_url(reqwest::Url::parse(UPSTREAM_URL).expect("test URL"));
    let detail = format_upstream_request_error(&error);
    assert_diagnostic_preserved(&detail);
    assert!(detail.contains("[url=https://127.0.0.1:8443/v1/responses]"));
    assert!(detail.contains("builder error"));
    for error in [
        ExecutionRuntimeTransportError::InvalidProxy(error),
        ExecutionRuntimeTransportError::ClientBuild(
            reqwest_source_error().with_url(reqwest::Url::parse(UPSTREAM_URL).expect("test URL")),
        ),
    ] {
        assert_diagnostic_preserved(&error.to_string());
        assert_diagnostic_preserved(&format!("{error:?}"));
        assert_diagnostic_preserved(&format_hyper_error_chain(&error));
    }
}

#[test]
fn wreq_formatter_protects_typed_sources_with_and_without_a_primary_uri() {
    let error = wreq_source_error();
    assert!(error.source().is_some());
    assert!(error.uri().is_none());
    assert_diagnostic_preserved(&format_wreq_upstream_request_error(&error));

    let error = error.with_uri(UPSTREAM_URL.parse().expect("test URI"));
    let detail = format_wreq_upstream_request_error(&error);
    assert_diagnostic_preserved(&detail);
    assert!(detail.contains("[uri=https://127.0.0.1:8443/v1/responses]"));
    assert!(detail.contains("builder error"));
    let error = ExecutionRuntimeTransportError::BrowserClientBuild(error);
    assert_diagnostic_preserved(&error.to_string());
    assert_diagnostic_preserved(&format!("{error:?}"));
    assert_diagnostic_preserved(&format_hyper_error_chain(&error));
}

#[tokio::test]
async fn request_formatters_preserve_http_status_and_decode_kind() {
    let response = http::Response::builder()
        .status(503)
        .body("")
        .expect("test response");
    let error = reqwest::Response::from(response)
        .error_for_status()
        .expect_err("503 status error")
        .with_url(reqwest::Url::parse(UPSTREAM_URL).expect("test URL"));
    let detail = format_upstream_request_error(&error);
    assert_url_secrets_removed(&detail);
    assert!(detail.contains("503 Service Unavailable"));
    assert!(detail.contains("127.0.0.1:8443/v1/responses"));
    assert_eq!(error.status().map(|status| status.as_u16()), Some(503));

    let response = http::Response::builder()
        .status(503)
        .body("")
        .expect("test response");
    let error = wreq::Response::from(response)
        .error_for_status()
        .expect_err("503 status error")
        .with_uri(UPSTREAM_URL.parse().expect("test URI"));
    let detail = format_wreq_upstream_request_error(&error);
    assert_url_secrets_removed(&detail);
    assert!(detail.contains("503 Service Unavailable"));
    assert!(detail.contains("127.0.0.1:8443/v1/responses"));
    assert_eq!(error.status().map(|status| status.as_u16()), Some(503));

    let error = reqwest::Response::from(http::Response::new("invalid json"))
        .json::<serde_json::Value>()
        .await
        .expect_err("JSON decode error");
    let detail = format_upstream_request_error(&error);
    assert!(detail.contains("[kind=decode]"));
    assert!(detail.contains("expected value at line 1 column 1"));

    let error = wreq::Response::from(http::Response::new("invalid json"))
        .json::<serde_json::Value>()
        .await
        .expect_err("JSON decode error");
    let detail = format_wreq_upstream_request_error(&error);
    assert!(detail.contains("[kind=decode]"));
    assert!(detail.contains("expected value at line 1 column 1"));
}
