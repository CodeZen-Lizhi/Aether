use aether_ai_serving::AiRequestGzipPolicy;
use serde_json::Value;

use super::state::GatewayProviderTransportSnapshot;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct TransportRequestEncodingPolicy {
    pub content_encoding: Option<String>,
    pub request_gzip: Option<AiRequestGzipPolicy>,
}

pub(crate) fn resolve_transport_request_encoding_policy(
    transport: &GatewayProviderTransportSnapshot,
) -> TransportRequestEncodingPolicy {
    let request_gzip = transport_request_gzip_policy_from_config(
        transport.endpoint.config.as_ref(),
    )
    .or_else(|| transport_request_gzip_policy_from_config(transport.provider.config.as_ref()));
    if request_gzip.is_some() {
        return TransportRequestEncodingPolicy {
            content_encoding: None,
            request_gzip,
        };
    }

    TransportRequestEncodingPolicy {
        content_encoding: None,
        request_gzip: None,
    }
}

fn transport_request_gzip_policy_from_config(
    config: Option<&Value>,
) -> Option<AiRequestGzipPolicy> {
    let object = config?.as_object()?;

    for key in ["request_gzip", "request_body_gzip"] {
        if let Some(policy) = object
            .get(key)
            .and_then(transport_request_gzip_policy_from_value)
        {
            return Some(policy);
        }
    }

    let enabled = first_config_bool(
        object,
        &["request_gzip_enabled", "request_body_gzip_enabled"],
    );
    let min_bytes = first_config_usize(
        object,
        &["request_gzip_min_bytes", "request_body_gzip_min_bytes"],
    );

    match (enabled, min_bytes) {
        (Some(false), _) => Some(AiRequestGzipPolicy {
            enabled: Some(false),
            min_bytes: None,
        }),
        (Some(true), min_bytes) => Some(AiRequestGzipPolicy {
            enabled: Some(true),
            min_bytes,
        }),
        (None, Some(min_bytes)) => Some(AiRequestGzipPolicy {
            enabled: Some(true),
            min_bytes: Some(min_bytes),
        }),
        (None, None) => None,
    }
}

fn transport_request_gzip_policy_from_value(value: &Value) -> Option<AiRequestGzipPolicy> {
    if let Some(enabled) = value.as_bool() {
        return Some(AiRequestGzipPolicy {
            enabled: Some(enabled),
            min_bytes: None,
        });
    }

    let object = value.as_object()?;
    let enabled = first_config_bool(object, &["enabled"]);
    let min_bytes = first_config_usize(object, &["min_bytes"]);

    match (enabled, min_bytes) {
        (Some(false), _) => Some(AiRequestGzipPolicy {
            enabled: Some(false),
            min_bytes: None,
        }),
        (Some(true), min_bytes) => Some(AiRequestGzipPolicy {
            enabled: Some(true),
            min_bytes,
        }),
        (None, Some(min_bytes)) => Some(AiRequestGzipPolicy {
            enabled: Some(true),
            min_bytes: Some(min_bytes),
        }),
        (None, None) => None,
    }
}

fn first_config_bool(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<bool> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(config_bool))
}

fn config_bool(value: &Value) -> Option<bool> {
    value.as_bool().or_else(|| {
        value.as_str().and_then(|text| {
            let normalized = text.trim();
            if normalized.eq_ignore_ascii_case("true") {
                Some(true)
            } else if normalized.eq_ignore_ascii_case("false") {
                Some(false)
            } else {
                None
            }
        })
    })
}

fn first_config_usize(object: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<usize> {
    keys.iter()
        .find_map(|key| object.get(*key).and_then(config_usize))
}

fn config_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .or_else(|| {
            value
                .as_str()
                .and_then(|text| text.trim().parse::<usize>().ok())
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aether_provider_transport::snapshot::{
        GatewayProviderTransportEndpoint, GatewayProviderTransportKey,
        GatewayProviderTransportProvider, GatewayProviderTransportSnapshot,
    };
    use serde_json::{json, Value};

    fn sample_transport(
        provider_type: &str,
        endpoint_api_format: &str,
        provider_config: Option<Value>,
        endpoint_config: Option<Value>,
    ) -> GatewayProviderTransportSnapshot {
        GatewayProviderTransportSnapshot {
            provider: GatewayProviderTransportProvider {
                id: "provider-1".to_string(),
                name: "Provider".to_string(),
                provider_type: provider_type.to_string(),
                website: None,
                is_active: true,
                enable_format_conversion: true,
                concurrent_limit: None,
                max_retries: None,
                proxy: None,
                request_timeout_secs: None,
                stream_first_byte_timeout_secs: None,
                config: provider_config,
            },
            endpoint: GatewayProviderTransportEndpoint {
                id: "endpoint-1".to_string(),
                provider_id: "provider-1".to_string(),
                api_format: endpoint_api_format.to_string(),
                api_family: None,
                endpoint_kind: None,
                is_active: true,
                base_url: "https://api.example.test".to_string(),
                header_rules: None,
                body_rules: None,
                max_retries: None,
                custom_path: None,
                config: endpoint_config,
                format_acceptance_config: None,
                proxy: None,
            },
            key: GatewayProviderTransportKey {
                id: "key-1".to_string(),
                provider_id: "provider-1".to_string(),
                name: "key".to_string(),
                auth_type: "api_key".to_string(),
                is_active: true,
                api_formats: None,
                auth_type_by_format: None,
                allow_auth_channel_mismatch_formats: None,
                allowed_models: None,
                capabilities: None,
                rate_multipliers: None,
                expires_at_unix_secs: None,
                proxy: None,
                fingerprint: None,
                upstream_metadata: None,
                decrypted_api_key: "secret".to_string(),
                decrypted_auth_config: None,
            },
        }
    }

    fn resolved_gzip_policy(
        transport: &GatewayProviderTransportSnapshot,
    ) -> Option<AiRequestGzipPolicy> {
        resolve_transport_request_encoding_policy(transport).request_gzip
    }

    fn resolved_content_encoding(transport: &GatewayProviderTransportSnapshot) -> Option<String> {
        resolve_transport_request_encoding_policy(transport).content_encoding
    }

    #[test]
    fn endpoint_request_gzip_policy_overrides_provider_policy() {
        let transport = sample_transport(
            "openai",
            "openai:responses",
            Some(json!({"request_gzip": false})),
            Some(json!({"request_gzip": {"enabled": true, "min_bytes": 1024}})),
        );

        assert_eq!(
            resolved_gzip_policy(&transport),
            Some(AiRequestGzipPolicy {
                enabled: Some(true),
                min_bytes: Some(1024),
            })
        );
    }

    #[test]
    fn endpoint_request_gzip_false_disables_provider_and_codex_defaults() {
        let transport = sample_transport(
            "codex",
            "openai:responses",
            Some(json!({"request_gzip": {"enabled": true, "min_bytes": 1024}})),
            Some(json!({"request_gzip": false})),
        );

        assert_eq!(
            resolved_gzip_policy(&transport),
            Some(AiRequestGzipPolicy {
                enabled: Some(false),
                min_bytes: None,
            })
        );
    }

    #[test]
    fn request_gzip_policy_supports_top_level_aliases() {
        let transport = sample_transport(
            "openai",
            "openai:responses",
            None,
            Some(json!({
                "request_body_gzip_enabled": true,
                "request_body_gzip_min_bytes": "4096"
            })),
        );

        assert_eq!(
            resolved_gzip_policy(&transport),
            Some(AiRequestGzipPolicy {
                enabled: Some(true),
                min_bytes: Some(4096),
            })
        );
    }

    #[test]
    fn request_gzip_policy_treats_min_bytes_only_as_enabled() {
        let transport = sample_transport(
            "openai",
            "openai:responses",
            None,
            Some(json!({"request_gzip_min_bytes": 1})),
        );

        assert_eq!(
            resolved_gzip_policy(&transport),
            Some(AiRequestGzipPolicy {
                enabled: Some(true),
                min_bytes: Some(1),
            })
        );
    }

    #[test]
    fn codex_responses_api_key_auth_does_not_enable_default_compression() {
        let transport = sample_transport("codex", "openai:responses", None, None);

        assert_eq!(resolved_content_encoding(&transport), None);
        assert_eq!(resolved_gzip_policy(&transport), None);
    }

    #[test]
    fn codex_image_endpoint_does_not_get_responses_request_gzip_policy() {
        let transport = sample_transport("codex", "openai:image", None, None);

        assert_eq!(resolved_content_encoding(&transport), None);
        assert_eq!(resolved_gzip_policy(&transport), None);
    }

    #[test]
    fn codex_compact_endpoint_does_not_get_default_request_gzip_policy() {
        let transport = sample_transport("codex", "openai:responses:compact", None, None);

        assert_eq!(resolved_content_encoding(&transport), None);
        assert_eq!(resolved_gzip_policy(&transport), None);
    }

    #[test]
    fn non_codex_endpoint_does_not_get_default_request_gzip_policy() {
        let transport = sample_transport("openai", "openai:responses", None, None);

        assert_eq!(resolved_content_encoding(&transport), None);
        assert_eq!(resolved_gzip_policy(&transport), None);
    }
}
