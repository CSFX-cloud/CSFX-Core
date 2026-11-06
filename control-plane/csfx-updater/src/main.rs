mod config;
mod etcd;
mod git_mirror;
mod health;
mod nix_build;
mod poller;
mod rollback;
mod updater;

use std::time::Duration;
use tokio::sync::watch;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    dotenvy::dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let cfg = config::Config::from_env()?;

    info!(
        poll_interval_secs = cfg.poll_interval_secs,
        infra_repo_github = %cfg.infra_repo_github,
        "csfx-updater started"
    );

    let cfg = std::sync::Arc::new(cfg);
    let cfg_poller = cfg.clone();
    let cfg_executor = cfg.clone();

    let poller_task = tokio::spawn(async move {
        run_poller_loop(&cfg_poller).await;
    });

    let executor_task = tokio::spawn(async move {
        run_executor_loop(&cfg_executor).await;
    });

    tokio::select! {
        _ = poller_task => tracing::error!("poller task exited unexpectedly"),
        _ = executor_task => tracing::error!("executor task exited unexpectedly"),
    }

    Ok(())
}

async fn run_poller_loop(cfg: &config::Config) {
    let mut last_etag: Option<String> = None;
    let interval = Duration::from_secs(cfg.poll_interval_secs);

    loop {
        match git_mirror::sync(&cfg.infra_repo_mirror_dir, &cfg.infra_repo_mirror_url).await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!(error = %e, "git mirror sync failed");
                tokio::time::sleep(interval).await;
                continue;
            }
        }

        let mut etcd = match etcd::Client::connect(cfg).await {
            Ok(c) => c,
            Err(e) => {
                tracing::error!(error = %e, "etcd connect failed in poller");
                tokio::time::sleep(interval).await;
                continue;
            }
        };

        match poller::poll_and_update(cfg, &mut etcd, &mut last_etag).await {
            Ok(Some(sha)) => info!(sha = %sha, "available_flake_rev updated"),
            Ok(None) => {}
            Err(e) => tracing::error!(error = %e, "poll failed"),
        }

        tokio::time::sleep(interval).await;
    }
}

async fn run_executor_loop(cfg: &config::Config) {
    let mut last_applied = String::new();
    let interval = Duration::from_secs(10);

    loop {
        tokio::time::sleep(interval).await;

        match execute_once(cfg, &last_applied).await {
            Ok(Some(rev)) => last_applied = rev,
            Ok(None) => {}
            Err(e) => tracing::error!(error = %e, "executor cycle failed"),
        }
    }
}

async fn execute_once(cfg: &config::Config, last_applied: &str) -> anyhow::Result<Option<String>> {
    let mut etcd = etcd::Client::connect(cfg).await?;

    let forced = etcd.get(etcd::FORCE_UPDATE_KEY).await?.as_deref() == Some("true");

    if !forced && etcd.get(etcd::PAUSED_KEY).await?.as_deref() == Some("true") {
        tracing::warn!("updater paused, skipping cycle");
        return Ok(None);
    }

    let desired = match etcd.get(etcd::DESIRED_FLAKE_REV_KEY).await? {
        Some(v) => v,
        None => {
            tracing::debug!("no desired flake rev in etcd, skipping cycle");
            return Ok(None);
        }
    };

    if desired.is_empty() {
        tracing::debug!("desired flake rev is empty, skipping cycle");
        return Ok(None);
    }

    if desired == last_applied {
        tracing::debug!(flake_rev = %desired, "already on desired rev, nothing to do");
        return Ok(None);
    }

    if forced {
        etcd.delete(etcd::FORCE_UPDATE_KEY).await?;
        info!(flake_rev = %desired, "forced update requested, bypassing pause");
    }

    if !is_valid_sha(&desired) {
        tracing::warn!(flake_rev = %desired, "rejected invalid flake rev");
        etcd.put(etcd::RESULT_KEY, "failed").await?;
        return Ok(Some(desired));
    }

    if !git_mirror::rev_exists(&cfg.infra_repo_mirror_dir, &desired).await? {
        tracing::warn!(flake_rev = %desired, "rev not found in mirror, triggering fetch");
        git_mirror::sync(&cfg.infra_repo_mirror_dir, &cfg.infra_repo_mirror_url).await?;

        if !git_mirror::rev_exists(&cfg.infra_repo_mirror_dir, &desired).await? {
            tracing::error!(flake_rev = %desired, "rev still not found after fetch");
            etcd.put(etcd::RESULT_KEY, "failed").await?;
            return Ok(Some(desired));
        }
    }

    info!(flake_rev = %desired, last_applied = %last_applied, "starting update");
    etcd.put(etcd::BUILD_STATUS_KEY, "building").await?;

    let (_cancel_tx, cancel_rx) = watch::channel(false);

    match nix_build::build(
        &cfg.infra_repo_mirror_dir,
        &desired,
        &cfg.nixos_config,
        cancel_rx,
    )
    .await
    {
        Ok(()) => {}
        Err(e) => {
            tracing::error!(error = %e, flake_rev = %desired, "nix build failed");
            etcd.put(etcd::BUILD_STATUS_KEY, "failed").await?;
            etcd.put(etcd::RESULT_KEY, "failed").await?;
            return Ok(Some(desired));
        }
    }

    match updater::switch(cfg, &desired).await {
        Ok(()) => {}
        Err(e) => {
            tracing::error!(error = %e, flake_rev = %desired, "nixos-rebuild switch failed");
            etcd.put(etcd::BUILD_STATUS_KEY, "failed").await?;
            etcd.put(etcd::RESULT_KEY, "failed").await?;
            return Ok(Some(desired));
        }
    }

    etcd.put(etcd::BUILD_STATUS_KEY, "verifying").await?;
    info!(
        flake_rev = %desired,
        timeout_secs = cfg.health_check_timeout_secs,
        "waiting for api-gateway to become healthy"
    );

    let healthy = health::wait_for_gateway(
        &cfg.gateway_url,
        cfg.health_check_timeout_secs,
        cfg.health_check_retry_interval_secs,
    )
    .await;

    if healthy {
        etcd.put(etcd::BUILD_STATUS_KEY, "ready").await?;
        etcd.put(etcd::RESULT_KEY, "success").await?;
        info!(flake_rev = %desired, "update verified and complete");
    } else {
        tracing::error!(
            flake_rev = %desired,
            timeout_secs = cfg.health_check_timeout_secs,
            "api-gateway did not become healthy after update, triggering rollback"
        );
        etcd.put(etcd::BUILD_STATUS_KEY, "rolling_back").await?;
        etcd.put(etcd::RESULT_KEY, "rolled_back").await?;

        if let Err(e) = rollback::rollback().await {
            tracing::error!(error = %e, "rollback failed");
            etcd.put(etcd::BUILD_STATUS_KEY, "rollback_failed").await?;
        } else {
            etcd.put(etcd::BUILD_STATUS_KEY, "rolled_back").await?;
        }
    }

    Ok(Some(desired))
}

fn is_valid_sha(rev: &str) -> bool {
    rev.len() == 40 && rev.chars().all(|c| c.is_ascii_hexdigit())
}

#[cfg(test)]
mod tests {
    use super::is_valid_sha;

    #[test]
    fn full_length_hex_revisions_are_accepted() {
        assert!(is_valid_sha("0123456789abcdef0123456789ABCDEF01234567"));
    }

    #[test]
    fn malformed_revisions_are_rejected() {
        assert!(!is_valid_sha(""));
        assert!(!is_valid_sha("abc123"));
        assert!(!is_valid_sha(&"a".repeat(41)));
        assert!(!is_valid_sha(&"g".repeat(40)));
        assert!(!is_valid_sha("main"));
    }
}
