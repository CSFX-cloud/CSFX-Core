use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub etcd_endpoints: Vec<String>,
    pub etcd_username: String,
    pub etcd_password: String,
    pub ghcr_org: String,
    pub ghcr_token: String,
    pub compose_file: String,
    pub poll_interval_secs: u64,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            etcd_endpoints: env::var("ETCD_ENDPOINTS")
                .unwrap_or_else(|_| "http://localhost:2379".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            etcd_username: env::var("ETCD_USERNAME").unwrap_or_else(|_| "csf".to_string()),
            etcd_password: env::var("ETCD_PASSWORD").context("ETCD_PASSWORD must be set")?,
            ghcr_org: env::var("GHCR_ORG").context("GHCR_ORG must be set")?,
            ghcr_token: env::var("GHCR_TOKEN").context("GHCR_TOKEN must be set")?,
            compose_file: env::var("COMPOSE_FILE")
                .unwrap_or_else(|_| "/etc/csf-core/docker-compose.yml".to_string()),
            poll_interval_secs: env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(30),
        })
    }
}
