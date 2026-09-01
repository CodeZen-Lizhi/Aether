#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderLocalEmbeddingSupport {
    None,
    AnyKnown,
    OpenAi,
    Gemini,
    Jina,
    Doubao,
    Aliyun,
}

impl ProviderLocalEmbeddingSupport {
    pub fn supports_api_format(self, api_format: &str) -> bool {
        let api_format = aether_ai_formats::normalize_api_format_alias(api_format);
        match self {
            Self::None => false,
            Self::AnyKnown => matches!(
                api_format.as_str(),
                "openai:embedding"
                    | "openai:rerank"
                    | "gemini:embedding"
                    | "jina:embedding"
                    | "jina:rerank"
                    | "doubao:embedding"
                    | "aliyun:multimodal_embedding"
            ),
            Self::OpenAi => matches!(api_format.as_str(), "openai:embedding" | "openai:rerank"),
            Self::Gemini => api_format == "gemini:embedding",
            Self::Jina => matches!(api_format.as_str(), "jina:embedding" | "jina:rerank"),
            Self::Doubao => api_format == "doubao:embedding",
            Self::Aliyun => api_format == "aliyun:multimodal_embedding",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProviderRuntimePolicy {
    pub supports_model_fetch: bool,
    pub supports_local_openai_chat_transport: bool,
    pub supports_local_same_format_transport: bool,
    pub local_embedding_support: ProviderLocalEmbeddingSupport,
}

impl ProviderRuntimePolicy {
    pub const fn standard() -> Self {
        Self {
            supports_model_fetch: true,
            supports_local_openai_chat_transport: true,
            supports_local_same_format_transport: true,
            local_embedding_support: ProviderLocalEmbeddingSupport::None,
        }
    }

    pub fn supports_local_embedding_transport(self, api_format: &str) -> bool {
        self.local_embedding_support.supports_api_format(api_format)
    }
}

const STANDARD_RUNTIME_POLICY: ProviderRuntimePolicy = ProviderRuntimePolicy::standard();
const CUSTOM_RUNTIME_POLICY: ProviderRuntimePolicy = ProviderRuntimePolicy {
    local_embedding_support: ProviderLocalEmbeddingSupport::AnyKnown,
    ..STANDARD_RUNTIME_POLICY
};

pub fn provider_runtime_policy(provider_type: &str) -> ProviderRuntimePolicy {
    match provider_type.trim().to_ascii_lowercase().as_str() {
        "custom" => CUSTOM_RUNTIME_POLICY,
        _ => STANDARD_RUNTIME_POLICY,
    }
}

pub fn provider_type_supports_model_fetch(provider_type: &str) -> bool {
    provider_runtime_policy(provider_type).supports_model_fetch
}

pub fn provider_type_supports_local_openai_chat_transport(provider_type: &str) -> bool {
    provider_runtime_policy(provider_type).supports_local_openai_chat_transport
}

pub fn provider_type_supports_local_same_format_transport(provider_type: &str) -> bool {
    provider_runtime_policy(provider_type).supports_local_same_format_transport
}

pub fn provider_type_supports_local_embedding_transport(
    provider_type: &str,
    api_format: &str,
) -> bool {
    provider_runtime_policy(provider_type).supports_local_embedding_transport(api_format)
}

#[cfg(test)]
mod tests {
    use super::{
        provider_runtime_policy, provider_type_supports_local_embedding_transport,
        ProviderLocalEmbeddingSupport, ProviderRuntimePolicy,
    };

    #[test]
    fn custom_supports_any_known_embedding_format() {
        for api_format in [
            "openai:embedding",
            "openai:rerank",
            "gemini:embedding",
            "jina:embedding",
            "jina:rerank",
            "doubao:embedding",
            "aliyun:multimodal_embedding",
        ] {
            assert!(
                provider_type_supports_local_embedding_transport("custom", api_format),
                "custom should support {api_format}"
            );
        }

        assert!(!provider_type_supports_local_embedding_transport(
            "custom",
            "openai:chat"
        ));
        assert!(!provider_type_supports_local_embedding_transport(
            "custom",
            "claude:messages"
        ));

        assert!(provider_type_supports_local_embedding_transport(
            " CUSTOM ",
            "GEMINI:EMBEDDING"
        ));
    }

    #[test]
    fn custom_supports_model_fetch_and_local_transports() {
        let policy = provider_runtime_policy("custom");
        assert!(policy.supports_model_fetch);
        assert!(policy.supports_local_openai_chat_transport);
        assert!(policy.supports_local_same_format_transport);
        assert_eq!(
            policy.local_embedding_support,
            ProviderLocalEmbeddingSupport::AnyKnown
        );
    }

    #[test]
    fn unknown_types_use_the_standard_policy() {
        let policy = provider_runtime_policy("anything-else");
        assert_eq!(policy, ProviderRuntimePolicy::standard());
        assert!(policy.supports_model_fetch);
        assert!(policy.supports_local_openai_chat_transport);
        assert!(policy.supports_local_same_format_transport);
        assert_eq!(
            policy.local_embedding_support,
            ProviderLocalEmbeddingSupport::None
        );
        assert!(!provider_type_supports_local_embedding_transport(
            "anything-else",
            "openai:embedding"
        ));
    }
}
