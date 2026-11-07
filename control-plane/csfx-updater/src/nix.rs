use anyhow::{bail, Result};
use std::time::Duration;
use tokio::process::Command;

const SYSTEM_PROFILE: &str = "/nix/var/nix/profiles/system";
const SWITCH_UNIT: &str = "csfx-switch";
const ROLLBACK_UNIT: &str = "csfx-rollback";

async fn run(program: &str, args: &[&str]) -> Result<()> {
    let output = Command::new(program).args(args).output().await?;
    if !output.status.success() {
        bail!(
            "{program} failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

pub async fn current_system() -> Result<String> {
    Ok(tokio::fs::canonicalize("/run/current-system")
        .await?
        .to_string_lossy()
        .into_owned())
}

pub async fn realise(toplevel: &str, substituters: &[String]) -> Result<()> {
    run(
        "nix-store",
        &[
            "--realise",
            toplevel,
            "--option",
            "substituters",
            &substituters.join(" "),
            "--option",
            "require-sigs",
            "true",
        ],
    )
    .await
}

pub async fn set_profile(toplevel: &str) -> Result<()> {
    run("nix-env", &["-p", SYSTEM_PROFILE, "--set", toplevel]).await
}

async fn switch_with_unit(toplevel: &str, unit: &str) -> Result<()> {
    let activator = format!("{toplevel}/bin/switch-to-configuration");
    run(
        "systemd-run",
        &[
            "--unit",
            unit,
            "--wait",
            "--collect",
            "--quiet",
            &activator,
            "switch",
        ],
    )
    .await
}

pub async fn switch(toplevel: &str) -> Result<()> {
    switch_with_unit(toplevel, SWITCH_UNIT).await
}

pub async fn switch_back(toplevel: &str) -> Result<()> {
    switch_with_unit(toplevel, ROLLBACK_UNIT).await
}

async fn unit_active(unit: &str) -> bool {
    Command::new("systemctl")
        .args(["is-active", "--quiet", unit])
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false)
}

pub async fn wait_until_idle() {
    for unit in [SWITCH_UNIT, ROLLBACK_UNIT] {
        while unit_active(unit).await {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

pub async fn service_active(unit: &str) -> bool {
    unit_active(unit).await
}
