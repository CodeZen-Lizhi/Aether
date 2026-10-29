const MODEL_FETCH_INTERVAL_MINUTES_DEFAULT: u64 = 1440;
const MODEL_FETCH_INTERVAL_MINUTES_MIN: u64 = 60;
const MODEL_FETCH_INTERVAL_MINUTES_MAX: u64 = 10080;

pub fn model_fetch_interval_minutes() -> u64 {
    std::env::var("MODEL_FETCH_INTERVAL_MINUTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| {
            value.clamp(
                MODEL_FETCH_INTERVAL_MINUTES_MIN,
                MODEL_FETCH_INTERVAL_MINUTES_MAX,
            )
        })
        .unwrap_or(MODEL_FETCH_INTERVAL_MINUTES_DEFAULT)
}

#[cfg(test)]
mod tests {
    use super::model_fetch_interval_minutes;

    struct TestEnvVarGuard {
        key: &'static str,
        previous: Option<String>,
    }

    impl Drop for TestEnvVarGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.previous.as_deref() {
                std::env::set_var(self.key, previous);
            } else {
                std::env::remove_var(self.key);
            }
        }
    }

    fn set_test_env_var(key: &'static str, value: &str) -> TestEnvVarGuard {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value);
        TestEnvVarGuard { key, previous }
    }

    #[test]
    fn interval_minutes_clamps_to_supported_bounds() {
        let _interval = set_test_env_var("MODEL_FETCH_INTERVAL_MINUTES", "5");
        assert_eq!(model_fetch_interval_minutes(), 60);

        let _interval = set_test_env_var("MODEL_FETCH_INTERVAL_MINUTES", "20000");
        assert_eq!(model_fetch_interval_minutes(), 10080);
    }
}
