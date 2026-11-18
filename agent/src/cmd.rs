use anyhow::{Context, Result};
use std::path::Path;
use std::process::Output;
use tokio::process::Command;

const SYS_CLASS_NET: &str = "/sys/class/net";
const ALREADY_EXISTS: &str = "File exists";
const MISSING_DEVICE: &str = "Cannot find device";

pub async fn run(program: &str, args: &[&str]) -> Result<Output> {
    Command::new(program)
        .args(args)
        .output()
        .await
        .with_context(|| format!("failed to execute {program}"))
}

pub async fn run_ok(program: &str, args: &[&str], tolerated: &[&str]) -> Result<()> {
    let output = run(program, args).await?;
    if output.status.success() {
        return Ok(());
    }
    let stderr = String::from_utf8_lossy(&output.stderr);
    if tolerated.iter().any(|message| stderr.contains(message)) {
        return Ok(());
    }
    anyhow::bail!("{program} command failed args={args:?} stderr={stderr}")
}

pub async fn stdout(program: &str, args: &[&str]) -> Result<String> {
    let output = run(program, args).await?;
    anyhow::ensure!(
        output.status.success(),
        "{program} command failed args={args:?} stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub async fn ip(args: &[&str]) -> Result<()> {
    run_ok("ip", args, &[ALREADY_EXISTS]).await
}

pub async fn wg(args: &[&str]) -> Result<()> {
    run_ok("wg", args, &[]).await
}

pub async fn delete_link(iface: &str) -> Result<()> {
    run_ok("ip", &["link", "delete", iface], &[MISSING_DEVICE]).await
}

pub async fn link_exists(iface: &str) -> Result<bool> {
    Ok(run("ip", &["link", "show", iface]).await?.status.success())
}

pub async fn has_ipv4_address(iface: &str) -> Result<bool> {
    let output = run("ip", &["-4", "-o", "addr", "show", "dev", iface]).await?;
    Ok(output.status.success() && !output.stdout.is_empty())
}

pub async fn address_assigned(iface: &str, address: &str) -> Result<bool> {
    let output = run("ip", &["-4", "-o", "addr", "show", "dev", iface]).await?;
    Ok(output.status.success() && has_address(&String::from_utf8_lossy(&output.stdout), address))
}

fn has_address(listing: &str, wanted: &str) -> bool {
    listing.split_whitespace().any(|field| field == wanted)
}

pub fn existing_links() -> std::collections::HashSet<String> {
    std::fs::read_dir(Path::new(SYS_CLASS_NET))
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeTools;

    #[tokio::test]
    async fn tolerated_messages_are_not_errors() {
        let _tools =
            FakeTools::install(&[("ip", "echo 'RTNETLINK answers: File exists' >&2\nexit 2")]);
        ip(&["link", "add", "name", "br0", "type", "bridge"])
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn other_failures_name_the_command() {
        let _tools = FakeTools::install(&[("ip", "echo 'bad' >&2\nexit 2")]);
        let error = ip(&["link", "add"]).await.unwrap_err().to_string();
        assert!(error.contains("ip command failed"));
        assert!(error.contains("bad"));
    }

    #[tokio::test]
    async fn missing_links_are_tolerated_on_delete() {
        {
            let _tools =
                FakeTools::install(&[("ip", "echo 'Cannot find device \"x\"' >&2\nexit 1")]);
            delete_link("x").await.unwrap();
        }
        let _tools = FakeTools::install(&[("ip", "echo 'Operation not permitted' >&2\nexit 1")]);
        assert!(delete_link("x").await.is_err());
    }

    #[tokio::test]
    async fn link_existence_follows_the_exit_status() {
        {
            let _tools = FakeTools::install(&[("ip", "exit 0")]);
            assert!(link_exists("br0").await.unwrap());
        }
        let _tools = FakeTools::install(&[("ip", "exit 1")]);
        assert!(!link_exists("br0").await.unwrap());
    }

    #[test]
    fn address_is_matched_as_a_whole_field() {
        let listing = "5: wgmgmt0    inet 10.100.0.7/32 scope global wgmgmt0";
        assert!(has_address(listing, "10.100.0.7/32"));
        assert!(!has_address(listing, "10.100.0.7/3"));
        assert!(!has_address(listing, "10.100.0.8/32"));
        assert!(!has_address("", "10.100.0.7/32"));
    }

    #[tokio::test]
    async fn stdout_requires_success() {
        {
            let _tools = FakeTools::install(&[("ip", "echo listing")]);
            assert_eq!(stdout("ip", &["route"]).await.unwrap().trim(), "listing");
        }
        let _tools = FakeTools::install(&[("ip", "exit 1")]);
        assert!(stdout("ip", &["route"]).await.is_err());
    }
}
