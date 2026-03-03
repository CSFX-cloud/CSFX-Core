use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

const STATE_DIR: &str = "/var/lib/csf-daemon";
const CREDENTIALS_FILE: &str = "/var/lib/csf-daemon/credentials";
const CONFIG_FILE: &str = "/var/lib/csf-daemon/config.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    pub gateway_url: String,
    pub agent_id: Uuid,
    pub heartbeat_interval_secs: u64,
}

#[derive(Debug, Clone)]
pub struct Credentials {
    pub api_key: String,
}

pub fn is_registered() -> bool {
    Path::new(CREDENTIALS_FILE).exists() && Path::new(CONFIG_FILE).exists()
}

pub fn load_config() -> Result<DaemonConfig> {
    let data = std::fs::read_to_string(CONFIG_FILE)
        .context("Failed to read daemon config")?;
    serde_json::from_str(&data).context("Failed to parse daemon config")
}

pub fn save_config(config: &DaemonConfig) -> Result<()> {
    std::fs::create_dir_all(STATE_DIR).context("Failed to create state directory")?;
    let data = serde_json::to_string_pretty(config).context("Failed to serialize config")?;
    std::fs::write(CONFIG_FILE, data).context("Failed to write daemon config")
}

pub fn load_credentials() -> Result<Credentials> {
    let api_key = std::fs::read_to_string(CREDENTIALS_FILE)
        .context("Failed to read credentials file")?
        .trim()
        .to_string();
    Ok(Credentials { api_key })
}

pub fn save_credentials(api_key: &str) -> Result<()> {
    std::fs::create_dir_all(STATE_DIR).context("Failed to create state directory")?;
    std::fs::write(CREDENTIALS_FILE, api_key).context("Failed to write credentials")?;
    set_permissions_600(CREDENTIALS_FILE)
}

#[cfg(unix)]
fn set_permissions_600(path: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)
        .context("Failed to set credentials file permissions")
}

#[cfg(not(unix))]
fn set_permissions_600(_path: &str) -> Result<()> {
    Ok(())
}

