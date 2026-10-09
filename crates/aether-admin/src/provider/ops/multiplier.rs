use serde_json::Value;

/// 已精确匹配的上游密钥倍率；专属倍率覆盖默认倍率，不相乘。
#[derive(Debug, Clone, PartialEq)]
pub struct Sub2ApiKeyMultiplier {
    /// 上游分组标识，仅用于展示和关联，不用于跨站点匹配。
    pub group_id: i64,
    /// 上游提供的分组名称；缺失时不伪造名称。
    pub group_name: Option<String>,
    /// 有限且非负的当前用户生效倍率，零值有效。
    pub multiplier: f64,
}

/// 按完整密钥匹配并解析倍率；列表及专属倍率必须来自同一站点和账号。
pub fn resolve_sub2api_key_multiplier(
    secret: &str,
    keys: &[Value],
    groups: &[Value],
    user_rates: &serde_json::Map<String, Value>,
) -> Result<Sub2ApiKeyMultiplier, String> {
    if secret.is_empty() {
        return Err("本地密钥为空".to_string());
    }
    let matches = keys
        .iter()
        .filter(|key| key.get("key").and_then(Value::as_str) == Some(secret))
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return Err("未找到匹配的上游密钥".to_string());
    }
    if matches.len() != 1 {
        return Err("上游返回重复密钥，无法确定分组".to_string());
    }
    let key = matches[0];
    let group_id = key
        .get("group_id")
        .and_then(Value::as_i64)
        .filter(|id| *id > 0)
        .ok_or_else(|| "上游密钥没有有效分组".to_string())?;
    let group = key
        .get("group")
        .filter(|group| group.is_object())
        .or_else(|| {
            groups
                .iter()
                .find(|group| group.get("id").and_then(Value::as_i64) == Some(group_id))
        });
    let value = user_rates
        .get(&group_id.to_string())
        .or_else(|| group.and_then(|group| group.get("rate_multiplier")))
        .ok_or_else(|| "上游未返回该分组的有效倍率".to_string())?;
    let multiplier = value
        .as_f64()
        .filter(|rate| rate.is_finite() && *rate >= 0.0)
        .ok_or_else(|| "上游倍率必须是有限且非负的数值".to_string())?;
    Ok(Sub2ApiKeyMultiplier {
        group_id,
        group_name: group
            .and_then(|group| group.get("name"))
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        multiplier,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// 验证专属零倍率覆盖默认值，并且只匹配指定密钥。
    #[test]
    fn user_zero_override_wins_for_exact_key() {
        let keys = vec![
            json!({"key":"other", "group_id":2, "group":{"rate_multiplier":0.9}}),
            json!({"key":"wanted", "group_id":1, "group":{"name":"group", "rate_multiplier":0.5}}),
        ];
        let rates = json!({"1":0.0});
        let resolved =
            resolve_sub2api_key_multiplier("wanted", &keys, &[], rates.as_object().unwrap())
                .unwrap();
        assert_eq!(resolved.multiplier, 0.0);
        assert_eq!(resolved.group_id, 1);
    }

    /// 验证缺失嵌套分组可从可用分组补齐，非法专属值不能静默退回默认。
    #[test]
    fn group_fallback_and_invalid_override() {
        let keys = vec![json!({"key":"wanted", "group_id":1})];
        let groups = vec![json!({"id":1,"rate_multiplier":0.3})];
        assert_eq!(
            resolve_sub2api_key_multiplier("wanted", &keys, &groups, &serde_json::Map::new())
                .unwrap()
                .multiplier,
            0.3
        );
        let invalid = json!({"1":-1});
        assert!(resolve_sub2api_key_multiplier(
            "wanted",
            &keys,
            &groups,
            invalid.as_object().unwrap()
        )
        .is_err());
        assert!(
            resolve_sub2api_key_multiplier("wanted", &keys, &[], &serde_json::Map::new()).is_err()
        );
        assert!(
            resolve_sub2api_key_multiplier("missing", &keys, &groups, &serde_json::Map::new())
                .is_err()
        );
    }
}
