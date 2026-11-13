use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;

pub struct Config {
    pub etcd_endpoints: Vec<String>,
    pub poll_interval_secs: u64,
    pub state_dir: PathBuf,
    pub release_base_url: String,
    pub release_public_key_file: PathBuf,
    pub peer_substituters: Vec<String>,
    pub upstream_substituters: Vec<String>,
    pub gateway_url: Option<String>,
    pub health_check_timeout_secs: u64,
    pub health_check_retry_interval_secs: u64,
    pub wave_bake_secs: u64,
    pub wave_max_failure_percent: u64,
}

fn number(name: &str, default: u64) -> u64 {
    env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
}

fn words(name: &str, default: &str) -> Vec<String> {
    env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            etcd_endpoints: env::var("ETCD_ENDPOINTS")
                .unwrap_or_else(|_| "http://localhost:2379".to_string())
                .split(',')
                .map(|endpoint| endpoint.trim().to_string())
                .collect(),
            poll_interval_secs: number("POLL_INTERVAL_SECS", 120),
            state_dir: env::var("STATE_DIR")
                .unwrap_or_else(|_| "/var/lib/csfx-updater".to_string())
                .into(),
            release_base_url: env::var("RELEASE_BASE_URL")
                .context("RELEASE_BASE_URL must be set (e.g. https://github.com/CSFX-cloud/CSFX-Infra/releases)")?
                .trim_end_matches('/')
                .to_string(),
            release_public_key_file: env::var("RELEASE_PUBLIC_KEY_FILE")
                .unwrap_or_else(|_| "/etc/csfx/release-public-key".to_string())
                .into(),
            peer_substituters: words("PEER_SUBSTITUTERS", ""),
            upstream_substituters: words("UPSTREAM_SUBSTITUTERS", "https://cache.nixos.org"),
            gateway_url: env::var("GATEWAY_URL").ok().filter(|url| !url.is_empty()),
            health_check_timeout_secs: number("HEALTH_CHECK_TIMEOUT_SECS", 120),
            health_check_retry_interval_secs: number("HEALTH_CHECK_RETRY_INTERVAL_SECS", 5),
            wave_bake_secs: number("WAVE_BAKE_SECS", 600),
            wave_max_failure_percent: number("WAVE_MAX_FAILURE_PERCENT", 10),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MANAGED_KEYS: [&str; 9] = [
        "ETCD_ENDPOINTS",
        "POLL_INTERVAL_SECS",
        "STATE_DIR",
        "RELEASE_BASE_URL",
        "PEER_SUBSTITUTERS",
        "UPSTREAM_SUBSTITUTERS",
        "GATEWAY_URL",
        "HEALTH_CHECK_TIMEOUT_SECS",
        "HEALTH_CHECK_RETRY_INTERVAL_SECS",
    ];

    fn clear_environment() {
        for key in MANAGED_KEYS {
            env::remove_var(key);
        }
    }

    #[test]
    fn configuration_is_read_from_the_environment() {
        clear_environment();
        assert!(Config::from_env().is_err());

        env::set_var("RELEASE_BASE_URL", "https://example.invalid/releases/");
        let defaults = Config::from_env().unwrap();
        assert_eq!(defaults.etcd_endpoints, vec!["http://localhost:2379"]);
        assert_eq!(
            defaults.release_base_url,
            "https://example.invalid/releases"
        );
        assert_eq!(
            defaults.upstream_substituters,
            vec!["https://cache.nixos.org"]
        );
        assert!(defaults.peer_substituters.is_empty());
        assert!(defaults.gateway_url.is_none());
        assert_eq!(defaults.health_check_timeout_secs, 120);

        env::set_var("ETCD_ENDPOINTS", "http://a:2379, http://b:2379");
        env::set_var(
            "PEER_SUBSTITUTERS",
            "http://10.0.0.1:5000 http://10.0.0.2:5000",
        );
        env::set_var("HEALTH_CHECK_TIMEOUT_SECS", "not-a-number");
        let overridden = Config::from_env().unwrap();
        assert_eq!(
            overridden.etcd_endpoints,
            vec!["http://a:2379", "http://b:2379"]
        );
        assert_eq!(overridden.peer_substituters.len(), 2);
        assert_eq!(overridden.health_check_timeout_secs, 120);
        clear_environment();
    }
}
