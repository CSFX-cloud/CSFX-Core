use anyhow::{bail, Result};
use tracing::info;

use crate::config::Config;

const SERVICES: &[&str] = &[
    "api-gateway",
    "registry",
    "scheduler",
    "volume-manager",
    "failover-controller",
    "sdn-controller",
];

pub async fn verify_images(cfg: &Config, version: &str, ghcr_auth: Option<&str>) -> Result<()> {
    let client = reqwest::Client::new();

    for svc in SERVICES {
        let image = format!("{}/csf-ce-{}", cfg.ghcr_org, svc);
        let remote = remote_digest(&client, &image, version, ghcr_auth).await?;
        let local = local_digest(&format!("ghcr.io/{}/csf-ce-{}:{}", cfg.ghcr_org, svc, version))?;

        if remote != local {
            bail!(
                "digest mismatch for {}: remote={} local={}",
                svc, remote, local
            );
        }

        info!(service = svc, digest = %remote, "image verified");
    }

    Ok(())
}

async fn remote_digest(client: &reqwest::Client, image: &str, tag: &str, ghcr_auth: Option<&str>) -> Result<String> {
    let url = format!("https://ghcr.io/v2/{}/manifests/{}", image, tag);
    let mut req = client
        .head(&url)
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json");

    if let Some(auth) = ghcr_auth {
        req = req.header("Authorization", format!("Basic {}", auth));
    }

    let resp = req.send().await?;

    if !resp.status().is_success() {
        bail!("GHCR manifest request failed for {}: {}", image, resp.status());
    }

    resp.headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no docker-content-digest header for {}", image))
}

fn local_digest(image: &str) -> Result<String> {
    let output = std::process::Command::new("docker")
        .args(["inspect", "--format", "{{index .RepoDigests 0}}", image])
        .output()?;

    if !output.status.success() {
        bail!("docker inspect failed for {}", image);
    }

    let raw = String::from_utf8(output.stdout)?;
    raw.trim()
        .split('@')
        .nth(1)
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("could not parse digest from docker inspect output for {}", image))
}
