use std::time::Duration;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

const TRIGGER_FILE: &str = "/var/lib/csf/update_trigger";
const MAX_JITTER_SECS: u64 = 300;

pub async fn handle(agent_id: Uuid, desired_flake_rev: &str, current_flake_rev: &str) {
    if desired_flake_rev == current_flake_rev {
        return;
    }

    if !is_valid_sha(desired_flake_rev) {
        warn!(flake_rev = %desired_flake_rev, "received invalid flake rev in heartbeat response");
        return;
    }

    let jitter = jitter_delay(agent_id);
    info!(
        flake_rev = %desired_flake_rev,
        jitter_secs = jitter,
        "update signal received, waiting before writing trigger"
    );

    tokio::time::sleep(Duration::from_secs(jitter)).await;

    if let Err(e) = write_trigger(desired_flake_rev).await {
        warn!(error = %e, flake_rev = %desired_flake_rev, "failed to write update trigger file");
    } else {
        info!(flake_rev = %desired_flake_rev, "update trigger written");
    }
}

async fn write_trigger(flake_rev: &str) -> anyhow::Result<()> {
    if let Some(parent) = std::path::Path::new(TRIGGER_FILE).parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(TRIGGER_FILE, flake_rev).await?;
    Ok(())
}

fn jitter_delay(agent_id: Uuid) -> u64 {
    let bytes = agent_id.as_bytes();
    let val = u64::from_le_bytes(bytes[..8].try_into().unwrap_or([0u8; 8]));
    val % MAX_JITTER_SECS
}

fn is_valid_sha(rev: &str) -> bool {
    rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit())
}
