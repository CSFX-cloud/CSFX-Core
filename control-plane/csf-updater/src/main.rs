mod config;
mod etcd;
mod secret;
mod updater;
mod verify;

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

    let desired = match etcd.get(etcd::DESIRED_VERSION_KEY).await? {
        Some(v) => v,
        None => return Ok(None),
    };

    if desired.is_empty() || desired == last_applied {
        return Ok(None);
    }

    if !is_valid_version(&desired) {
        tracing::warn!(version = %desired, "rejected invalid version string");
        etcd.put(etcd::RESULT_KEY, "failed").await?;
        return Ok(Some(desired));
    }

    info!(version = %desired, last_applied = %last_applied, "starting update");
    etcd.put(etcd::RESULT_KEY, "in_progress").await?;

    match updater::run(cfg, &desired, &mut etcd).await {
        Ok(()) => {
            etcd.put(etcd::RESULT_KEY, "success").await?;
            info!(version = %desired, "update complete");
            Ok(Some(desired))
        }
        Err(e) => {
            tracing::error!(error = %e, version = %desired, "update failed");
            etcd.put(etcd::RESULT_KEY, "failed").await?;
            Ok(Some(desired))
        }
    }
}

fn is_valid_version(v: &str) -> bool {
    let v = v.trim_start_matches('v');
    let (base, _pre) = match v.split_once('-') {
        Some((b, p)) => (b, Some(p)),
        None => (v, None),
    };
    let parts: Vec<&str> = base.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
}
