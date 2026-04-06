mod config;
mod etcd;
mod updater;

use std::time::Duration;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::from_env()?;
    let poll_interval = Duration::from_secs(cfg.poll_interval_secs);

    info!(poll_interval_secs = cfg.poll_interval_secs, "csf-updater started");

    let mut last_applied = String::new();

    loop {
        match run_once(&cfg, &last_applied).await {
            Ok(Some(version)) => {
                last_applied = version;
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!(error = %e, "update cycle error");
            }
        }
        tokio::time::sleep(poll_interval).await;
    }
}

async fn run_once(cfg: &config::Config, last_applied: &str) -> anyhow::Result<Option<String>> {
    let mut etcd = etcd::Client::connect(cfg).await?;

    if etcd.get(etcd::PAUSED_KEY).await?.as_deref() == Some("true") {
        tracing::info!("updates paused, skipping");
        return Ok(None);
    }

    let desired = match etcd.get(etcd::DESIRED_FLAKE_REV_KEY).await? {
        Some(v) => v,
        None => return Ok(None),
    };

    if desired.is_empty() || desired == last_applied {
        return Ok(None);
    }

    if !is_valid_sha(&desired) {
        tracing::warn!(flake_rev = %desired, "rejected invalid flake rev");
        etcd.put(etcd::RESULT_KEY, "failed").await?;
        return Ok(Some(desired));
    }

    info!(flake_rev = %desired, last_applied = %last_applied, "starting update");
    etcd.put(etcd::BUILD_STATUS_KEY, "building").await?;

    match updater::run(cfg, &desired, &mut etcd).await {
        Ok(()) => {
            etcd.put(etcd::BUILD_STATUS_KEY, "ready").await?;
            etcd.put(etcd::RESULT_KEY, "success").await?;
            info!(flake_rev = %desired, "update complete");
            Ok(Some(desired))
        }
        Err(e) => {
            tracing::error!(error = %e, flake_rev = %desired, "update failed");
            etcd.put(etcd::BUILD_STATUS_KEY, "failed").await?;
            etcd.put(etcd::RESULT_KEY, "failed").await?;
            Ok(Some(desired))
        }
    }
}

fn is_valid_sha(rev: &str) -> bool {
    rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit())
}
