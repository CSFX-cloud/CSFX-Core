use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub etcd_endpoints: Vec<String>,
    pub ghcr_org: String,
    pub compose_file: String,
    pub poll_interval_secs: u64,
    pub secret_encryption_key: String,
    pub binary_dir: String,
    pub github_release_base_url: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            etcd_endpoints: env::var("ETCD_ENDPOINTS")
                .unwrap_or_else(|_| "http://localhost:2379".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            ghcr_org: env::var("GHCR_ORG").context("GHCR_ORG must be set")?,
            compose_file: env::var("COMPOSE_FILE")
                .unwrap_or_else(|_| "/etc/csf-core/docker-compose.yml".to_string()),
            poll_interval_secs: env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
            secret_encryption_key: env::var("SECRET_ENCRYPTION_KEY")
                .context("SECRET_ENCRYPTION_KEY must be set")?,
            binary_dir: env::var("BINARY_DIR")
                .unwrap_or_else(|_| "/usr/local/bin".to_string()),
            github_release_base_url: env::var("GITHUB_RELEASE_BASE_URL")
                .unwrap_or_else(|_| "https://github.com/csfx-cloud/CSF-Core/releases/download".to_string()),
        })
    }
}
