use serde_json::Value;

pub const ABANDONED_KEY: &str = "abandoned";

pub fn abandoned_attempts(versions: Option<&Value>) -> u64 {
    versions
        .and_then(|recorded| recorded.get(ABANDONED_KEY))
        .and_then(Value::as_u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn counts_abandoned_rollouts_and_ignores_version_maps() {
        assert_eq!(abandoned_attempts(None), 0);
        assert_eq!(abandoned_attempts(Some(&json!({"v/A": 3}))), 0);
        assert_eq!(abandoned_attempts(Some(&json!({ ABANDONED_KEY: 2 }))), 2);
    }
}
