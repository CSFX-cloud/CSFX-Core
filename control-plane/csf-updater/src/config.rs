use anyhow::{Context, Result};
use std::env;

pub struct Config {
    pub etcd_endpoints: Vec<String>,
    pub poll_interval_secs: u64,
    pub infra_repo_mirror_dir: String,
    pub infra_repo_mirror_url: String,
    pub infra_repo_github: String,
    pub infra_repo_branch: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            etcd_endpoints: env::var("ETCD_ENDPOINTS")
                .unwrap_or_else(|_| "http://localhost:2379".to_string())
                .split(',')
                .map(|s| s.trim().to_string())
                .collect(),
            poll_interval_secs: env::var("POLL_INTERVAL_SECS")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(120),
            infra_repo_mirror_dir: env::var("INFRA_REPO_MIRROR_DIR")
                .unwrap_or_else(|_| "/var/lib/csf-updater/infra.git".to_string()),
            infra_repo_mirror_url: env::var("INFRA_REPO_MIRROR_URL")
                .context("INFRA_REPO_MIRROR_URL must be set")?,
            infra_repo_github: env::var("INFRA_REPO_GITHUB")
                .context("INFRA_REPO_GITHUB must be set (e.g. csfx-cloud/CSFX-Infra)")?,
            infra_repo_branch: env::var("INFRA_REPO_BRANCH")
                .unwrap_or_else(|_| "main".to_string()),
        })
    }
}
