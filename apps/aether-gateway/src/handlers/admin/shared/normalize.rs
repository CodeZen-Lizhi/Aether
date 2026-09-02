use crate::handlers::shared::normalize_ip_rules;

pub(crate) fn normalize_admin_user_string_list(
    value: Option<Vec<String>>,
    field_name: &str,
) -> Result<Option<Vec<String>>, String> {
    let Some(values) = value else {
        return Ok(None);
    };
    let mut normalized = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for item in values {
        let item = item.trim();
        if item.is_empty() {
            return Err(format!("{field_name} 不能为空"));
        }
        if seen.insert(item.to_string()) {
            normalized.push(item.to_string());
        }
    }
    Ok(Some(normalized))
}

pub(crate) fn normalize_admin_user_api_formats(
    value: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, String> {
    let Some(values) = value else {
        return Ok(None);
    };
    let mut normalized = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for item in values {
        let item = item.trim();
        if item.is_empty() {
            return Err("allowed_api_formats 不能为空".to_string());
        }
        if !looks_like_admin_api_format_signature(item) {
            return Err(format!("allowed_api_formats 格式无效: {item}"));
        }
        let Some(normalized_item) = crate::api::ai::normalize_admin_endpoint_signature(item) else {
            return Err(format!("allowed_api_formats 格式无效: {item}"));
        };
        let normalized_item = normalized_item.to_string();
        if seen.insert(normalized_item.clone()) {
            normalized.push(normalized_item);
        }
    }
    Ok(Some(normalized))
}

fn looks_like_admin_api_format_signature(value: &str) -> bool {
    value
        .split_once(':')
        .is_some_and(|(family, kind)| !family.trim().is_empty() && !kind.trim().is_empty())
}

pub(crate) fn normalize_admin_user_ip_rules(
    value: Option<Vec<String>>,
) -> Result<Option<Vec<String>>, String> {
    normalize_ip_rules(value)
}

pub(crate) fn normalize_admin_list_policy_mode(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "inherit" | "unrestricted" | "specific" | "deny_all" => {
            Ok(value.trim().to_ascii_lowercase())
        }
        _ => Err("权限列表模式不合法".to_string()),
    }
}

pub(crate) fn normalize_admin_rate_limit_policy_mode(value: &str) -> Result<String, String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "inherit" | "system" | "custom" => Ok(value.trim().to_ascii_lowercase()),
        _ => Err("限速模式不合法".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_admin_user_api_formats;

    #[test]
    fn admin_user_api_formats_accept_current_canonical_signatures() {
        assert_eq!(
            normalize_admin_user_api_formats(Some(vec![
                " OPENAI:RESPONSES ".to_string(),
                "claude:messages".to_string(),
                "gemini:generate_content".to_string(),
                "jina:rerank".to_string(),
                "openai:responses".to_string(),
            ]))
            .expect("formats should normalize"),
            Some(vec![
                "openai:responses".to_string(),
                "claude:messages".to_string(),
                "gemini:generate_content".to_string(),
                "jina:rerank".to_string(),
            ])
        );
    }

    #[test]
    fn admin_user_api_formats_reject_unsupported_signatures() {
        for unsupported in [
            "claude",
            "openai",
            "unknown:chat",
            "openai:unknown",
            "gemini:generate",
        ] {
            assert!(
                normalize_admin_user_api_formats(Some(vec![unsupported.to_string()])).is_err(),
                "{unsupported} should be rejected"
            );
        }
    }
}
