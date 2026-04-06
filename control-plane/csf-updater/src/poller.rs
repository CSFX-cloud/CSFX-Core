use anyhow::Result;
use reqwest::header::{ETAG, IF_NONE_MATCH};
use serde::Deserialize;
use tracing::info;

use crate::config::Config;
use crate::etcd;

#[derive(Debug, Deserialize)]
struct GitHubCommit {
    sha: String,
}

pub async fn poll_and_update(cfg: &Config, etcd: &mut etcd::Client, last_etag: &mut Option<String>) -> Result<Option<String>> {
    let url = format!(
        "https://api.github.com/repos/{}/commits/{}",
        cfg.infra_repo_github, cfg.infra_repo_branch
    );

    let mut req = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "csf-updater")
        .header("Accept", "application/vnd.github.v3+json");

    if let Some(etag) = last_etag.as_deref() {
        req = req.header(IF_NONE_MATCH, etag);
    }

    let resp = req.send().await?;

    if resp.status() == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(None);
    }

    if !resp.status().is_success() {
        anyhow::bail!("GitHub API returned {}", resp.status());
    }

    if let Some(etag) = resp.headers().get(ETAG) {
        *last_etag = Some(etag.to_str()?.to_string());
    }

    let commit: GitHubCommit = resp.json().await?;
    let sha = commit.sha;

    let current = etcd.get(etcd::AVAILABLE_FLAKE_REV_KEY).await?;
    if current.as_deref() == Some(&sha) {
        return Ok(None);
    }

    etcd.put(etcd::AVAILABLE_FLAKE_REV_KEY, &sha).await?;
    info!(sha = %sha, "new flake rev available");

    Ok(Some(sha))
}
