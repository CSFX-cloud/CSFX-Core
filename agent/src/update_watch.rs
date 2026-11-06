use std::time::Duration;
use tokio::fs;
use tracing::{info, warn};
use uuid::Uuid;

const TRIGGER_FILE: &str = "/var/lib/csfx/update_trigger";
const HEARTBEAT_COUNTER_FILE: &str = "/var/lib/csfx/post_update_heartbeats";
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

pub async fn write_heartbeat_counter(count: u32) {
    if let Some(parent) = std::path::Path::new(HEARTBEAT_COUNTER_FILE).parent() {
        let _ = fs::create_dir_all(parent).await;
    }
    let _ = fs::write(HEARTBEAT_COUNTER_FILE, count.to_string()).await;
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_forty_hex_characters_are_a_valid_sha() {
        assert!(is_valid_sha(&"a".repeat(40)));
        assert!(is_valid_sha("0123456789abcdefABCDEF0123456789abcdef01"));
        assert!(!is_valid_sha(&"a".repeat(39)));
        assert!(!is_valid_sha(&"g".repeat(40)));
    }

    #[test]
    fn jitter_is_deterministic_and_bounded() {
        let id = Uuid::from_u128(0xdead_beef_1234_5678_9abc_def0_1122_3344);
        assert_eq!(jitter_delay(id), jitter_delay(id));
        assert!(jitter_delay(id) < MAX_JITTER_SECS);
        assert_eq!(jitter_delay(Uuid::nil()), 0);
    }

    #[tokio::test]
    async fn handle_ignores_identical_and_invalid_revisions() {
        let id = Uuid::new_v4();
        handle(id, "abc", "abc").await;
        handle(id, "not-a-sha", "abc").await;
    }
}
