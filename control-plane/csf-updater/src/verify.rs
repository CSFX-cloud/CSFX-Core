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

async fn exchange_token(client: &reqwest::Client, image: &str, basic_auth: &str) -> Result<String> {
    let url = format!(
        "https://ghcr.io/token?scope=repository:{}:pull",
        image
    );
    let resp = client
        .get(&url)
        .header("Authorization", format!("Basic {}", basic_auth))
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("GHCR token exchange failed for {}: {}", image, resp.status());
    }

    let body: serde_json::Value = resp.json().await?;
    body["token"]
        .as_str()
        .map(|s| s.to_string())
        .ok_or_else(|| anyhow::anyhow!("no token in GHCR token response for {}", image))
}

async fn remote_digest(client: &reqwest::Client, image: &str, tag: &str, ghcr_auth: Option<&str>) -> Result<String> {
    let bearer = match ghcr_auth {
        Some(auth) => exchange_token(client, image, auth).await?,
        None => bail!("no GHCR auth configured"),
    };

    let url = format!("https://ghcr.io/v2/{}/manifests/{}", image, tag);
    let resp = client
        .head(&url)
        .header("Authorization", format!("Bearer {}", bearer))
        .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
        .send()
        .await?;

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
    let pull = std::process::Command::new("docker")
        .args(["pull", "--quiet", image])
        .output()?;

    if !pull.status.success() {
        bail!(
            "docker pull failed for {}: {}",
            image,
            String::from_utf8_lossy(&pull.stderr).trim()
        );
    }

    let output = std::process::Command::new("docker")
        .args(["image", "inspect", "--format", "{{json .RepoDigests}}", image])
        .output()?;

    if !output.status.success() {
        bail!("docker inspect failed for {}", image);
    }

    let raw = String::from_utf8(output.stdout)?;
    let digests: Vec<String> = serde_json::from_str(raw.trim())
        .map_err(|e| anyhow::anyhow!("failed to parse RepoDigests for {}: {}", image, e))?;

    digests
        .into_iter()
        .find_map(|d| d.split('@').nth(1).map(|s| s.to_string()))
        .ok_or_else(|| anyhow::anyhow!("no repo digest found for {}", image))
}
