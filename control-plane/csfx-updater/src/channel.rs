use anyhow::{bail, Context, Result};
use std::path::Path;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use crate::config::Config;

fn asset_url(base_url: &str, version: &str, file: &str) -> String {
    match version {
        "latest" => format!("{base_url}/latest/download/{file}"),
        _ => format!(
            "{base_url}/download/v{}/{file}",
            version.trim_start_matches('v')
        ),
    }
}

async fn get(url: &str) -> Result<reqwest::Response> {
    let response = reqwest::Client::new()
        .get(url)
        .header("User-Agent", "csfx-updater")
        .send()
        .await
        .with_context(|| format!("request to {url} failed"))?;
    if !response.status().is_success() {
        bail!("{url} returned {}", response.status());
    }
    Ok(response)
}

pub async fn fetch_manifest(cfg: &Config, version: &str, arch: &str) -> Result<(Vec<u8>, String)> {
    let file = format!("manifest-{arch}.json");
    let manifest = get(&asset_url(&cfg.release_base_url, version, &file))
        .await?
        .bytes()
        .await?
        .to_vec();
    let signature = get(&asset_url(
        &cfg.release_base_url,
        version,
        &format!("{file}.sig"),
    ))
    .await?
    .text()
    .await?;
    Ok((manifest, signature))
}

pub async fn download_bundle(
    cfg: &Config,
    version: &str,
    bundle: &str,
    destination: &Path,
) -> Result<()> {
    let mut response = get(&asset_url(&cfg.release_base_url, version, bundle)).await?;
    let mut file = tokio::fs::File::create(destination).await?;
    while let Some(chunk) = response.chunk().await? {
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    Ok(())
}

pub async fn extract_bundle(bundle: &Path, destination: &Path) -> Result<()> {
    tokio::fs::create_dir_all(destination).await?;
    let output = Command::new("tar")
        .arg("--zstd")
        .arg("-xf")
        .arg(bundle)
        .arg("-C")
        .arg(destination)
        .output()
        .await?;
    if !output.status.success() {
        bail!(
            "bundle extraction failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_and_latest_urls_follow_the_release_layout() {
        assert_eq!(
            asset_url(
                "https://example.invalid/releases",
                "0.3.0",
                "manifest-x86_64-linux.json"
            ),
            "https://example.invalid/releases/download/v0.3.0/manifest-x86_64-linux.json"
        );
        assert_eq!(
            asset_url(
                "https://example.invalid/releases",
                "latest",
                "manifest-x86_64-linux.json"
            ),
            "https://example.invalid/releases/latest/download/manifest-x86_64-linux.json"
        );
    }
}
