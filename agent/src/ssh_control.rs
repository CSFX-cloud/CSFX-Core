use tokio::process::Command;
use tracing::{info, warn};

const SSH_UNIT: &str = "sshd.service";

pub fn control_enabled() -> bool {
    std::env::var("CSFX_SSH_CONTROL").is_ok_and(|value| value == "1")
}

async fn is_active() -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", SSH_UNIT])
        .status()
        .await
        .is_ok_and(|status| status.success())
}

pub async fn apply(desired: bool) {
    if is_active().await == desired {
        return;
    }
    let action = if desired { "start" } else { "stop" };
    match Command::new("systemctl")
        .args([action, SSH_UNIT])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {
            info!(unit = SSH_UNIT, action = action, "ssh access changed");
        }
        Ok(output) => warn!(
            unit = SSH_UNIT,
            action = action,
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "failed to change ssh access"
        ),
        Err(err) => {
            warn!(unit = SSH_UNIT, action = action, error = %err, "failed to run systemctl")
        }
    }
}
