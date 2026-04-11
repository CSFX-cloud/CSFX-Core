use anyhow::{bail, Result};
use reqwest::header::{ETAG, IF_NONE_MATCH};
use serde::Deserialize;
use tracing::info;

use crate::config::Config;
use crate::etcd;

#[derive(Debug, Deserialize)]
struct GitHubTag {
    object: GitHubObject,
}

#[derive(Debug, Deserialize)]
struct GitHubObject {
    sha: String,
    #[serde(rename = "type")]
    kind: String,
}

#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
}

pub async fn poll_and_update(
    cfg: &Config,
    etcd: &mut etcd::Client,
    last_etag: &mut Option<String>,
) -> Result<Option<String>> {
    let desired_version = match etcd.get(etcd::DESIRED_VERSION_KEY).await? {
        Some(v) if !v.is_empty() => v,
        _ => return Ok(None),
    };

    let sha = resolve_version_to_sha(cfg, &desired_version, last_etag).await?;

    let current = etcd.get(etcd::AVAILABLE_FLAKE_REV_KEY).await?;
    if current.as_deref() == Some(sha.as_str()) {
        return Ok(None);
    }

    etcd.put(etcd::AVAILABLE_FLAKE_REV_KEY, &sha).await?;
    etcd.put(etcd::DESIRED_FLAKE_REV_KEY, &sha).await?;
    info!(version = %desired_version, sha = %sha, "resolved version to flake rev");

    Ok(Some(sha))
}

async fn resolve_version_to_sha(
    cfg: &Config,
    version: &str,
    last_etag: &mut Option<String>,
) -> Result<String> {
    let tag = format!("v{}", version.trim_start_matches('v'));
    let url = format!(
        "https://api.github.com/repos/{}/git/ref/tags/{}",
        cfg.infra_repo_github, tag
    );

    let client = reqwest::Client::new();
    let mut req = client
        .get(&url)
        .header("User-Agent", "csfx-updater")
        .header("Accept", "application/vnd.github.v3+json");

    if let Some(etag) = last_etag.as_deref() {
        req = req.header(IF_NONE_MATCH, etag);
    }

    let resp = req.send().await?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        bail!("tag not modified, no new sha available");
    }

    if !resp.status().is_success() {
        bail!(
            "GitHub API returned {} for tag {}",
            resp.status(),
            tag
        );
    }

    if let Some(etag) = resp.headers().get(ETAG) {
        *last_etag = Some(etag.to_str()?.to_string());
    }

    let tag_ref: GitHubTag = resp.json().await?;

    let sha = if tag_ref.object.kind == "tag" {
        dereference_tag(cfg, &tag_ref.object.sha).await?
    } else {
        tag_ref.object.sha
    };

    Ok(sha)
}

async fn dereference_tag(cfg: &Config, tag_sha: &str) -> Result<String> {
    let url = format!(
        "https://api.github.com/repos/{}/git/tags/{}",
        cfg.infra_repo_github, tag_sha
    );

    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "csfx-updater")
        .header("Accept", "application/vnd.github.v3+json")
        .send()
        .await?;

    if !resp.status().is_success() {
        bail!("GitHub API returned {} when dereferencing tag", resp.status());
    }

    let tag: GitHubTag = resp.json().await?;
    Ok(tag.object.sha)
}
