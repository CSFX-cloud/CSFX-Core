use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{error, info, warn};

use crate::reconcile::Reconciler;
use crate::system::{LiveMetricsCollector, SystemMetrics};
use crate::{backoff, client, config, fence, ssh_control, update_watch};

const HEARTBEAT_BACKOFF_CAP: Duration = Duration::from_secs(60);

struct Heartbeat {
    reconciler: Reconciler,
    metrics: Arc<Mutex<LiveMetricsCollector>>,
    failures: u32,
    fence: fence::FenceGuard,
}

impl Heartbeat {
    fn new(reconciler: Reconciler) -> Self {
        Self {
            reconciler,
            metrics: Arc::new(Mutex::new(LiveMetricsCollector::new())),
            failures: 0,
            fence: fence::FenceGuard::from_env(),
        }
    }

    async fn tick(&mut self) {
        self.reconciler.reconcile().await;
        let statuses = self.reconciler.container_statuses().await;
        self.reconciler.push_workload_stats().await;
        let Some(metrics) = self.sample_metrics().await else {
            return;
        };
        self.send(statuses, metrics).await;
    }

    async fn sample_metrics(&self) -> Option<SystemMetrics> {
        let collector = Arc::clone(&self.metrics);
        let sampled = tokio::task::spawn_blocking(move || {
            collector
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .sample()
        })
        .await;
        sampled
            .inspect_err(|error| error!(error = %error, "metrics collection task failed"))
            .ok()
    }

    async fn send(&mut self, statuses: Vec<client::ContainerStatus>, metrics: SystemMetrics) {
        let reconciler = &self.reconciler;
        let result = reconciler
            .client
            .heartbeat(reconciler.agent_id, Some(statuses), Some(metrics))
            .await;
        match result {
            Ok(response) => self.on_success(response).await,
            Err(error) => self.on_failure(error).await,
        }
    }

    async fn on_success(&mut self, response: client::HeartbeatResponse) {
        let agent_id = self.reconciler.agent_id;
        self.fence.record_contact(Instant::now());
        if self.failures > 0 {
            info!(agent_id = %agent_id, failures = self.failures, "Heartbeat recovered");
            self.failures = 0;
        }

        info!(
            agent_id = %agent_id,
            releases = response.worker_releases.len(),
            "heartbeat ok"
        );

        if ssh_control::control_enabled() {
            ssh_control::apply(response.ssh_enabled).await;
        }

        adopt_advertised_gateway(
            self.reconciler.client.gateway_url(),
            response.gateway_url.as_deref(),
        );

        if !response.worker_releases.is_empty() {
            tokio::spawn(update_watch::handle(agent_id, response.worker_releases));
        }
    }

    async fn on_failure(&mut self, error: anyhow::Error) {
        let agent_id = self.reconciler.agent_id;
        self.failures += 1;
        warn!(agent_id = %agent_id, failures = self.failures, error = %error, "Heartbeat failed");
        if self.fence.should_fence(&error).await {
            warn!(agent_id = %agent_id, "Isolated from control plane and all mesh peers, self-fencing all workloads");
            self.reconciler.fence_all().await;
        }
    }
}

fn next_tick_delay(period: Duration, failure_count: u32) -> Duration {
    if failure_count == 0 {
        return period;
    }
    backoff::equal_jitter(period, HEARTBEAT_BACKOFF_CAP.max(period), failure_count)
}

fn adopt_advertised_gateway(current: &str, advertised: Option<&str>) {
    let Some(advertised) =
        advertised.filter(|url| *url != current && !config::is_loopback_url(current))
    else {
        return;
    };
    match config::save_gateway_override(advertised) {
        Ok(()) => info!(gateway_url = %advertised, "Adopted advertised gateway address"),
        Err(e) => warn!(error = %e, "Failed to store advertised gateway address"),
    }
}

pub async fn run(
    reconciler: Reconciler,
    interval_secs: u64,
    mut assignment_signal: tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let period = Duration::from_secs(interval_secs.max(1));
    let mut interval = tokio::time::interval_at(
        tokio::time::Instant::now() + backoff::jitter(period),
        period,
    );
    let completed = Arc::clone(&reconciler.starts.completed);
    let agent_id = reconciler.agent_id;
    let mut heartbeat = Heartbeat::new(reconciler);

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            signal = assignment_signal.recv() => {
                if signal.is_none() {
                    warn!(agent_id = %agent_id, "assignment signal channel closed");
                    continue;
                }
                while assignment_signal.try_recv().is_ok() {}
                info!(agent_id = %agent_id, "assignment push received, reconciling immediately");
                interval.reset();
            }
            _ = completed.notified() => interval.reset(),
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
        heartbeat.tick().await;
        interval.reset_after(next_tick_delay(period, heartbeat.failures));
    }

    if heartbeat.failures > 0 {
        error!(
            failures = heartbeat.failures,
            "Agent shutting down with unresolved heartbeat failures"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reconcile::fixtures::{local_state, reconciler, serve};
    use axum::routing::{get, post};
    use axum::Router;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TickOutcome {
        failures: u32,
        heartbeats: usize,
    }

    async fn run_tick(
        response: Option<serde_json::Value>,
        guard: fence::FenceGuard,
        starting_failures: u32,
    ) -> (TickOutcome, fence::FenceGuard) {
        let _tools = crate::testing::FakeTools::install(&[
            ("wg", "exit 0"),
            ("ip", "exit 0"),
            ("nft", "exit 0"),
        ]);
        let heartbeats = Arc::new(AtomicUsize::new(0));
        let counter = heartbeats.clone();
        let empty = || async { axum::Json(serde_json::json!([])) };
        let app = Router::new()
            .route("/api/agents/self/volumes", get(empty))
            .route("/api/agents/self/workloads", get(empty))
            .route("/api/agents/mesh-peers", get(empty))
            .route("/api/resource-groups/agent/active-ids", get(empty))
            .route(
                "/api/registry/agents/{id}/heartbeat",
                post(move || {
                    let counter = counter.clone();
                    let response = response.clone();
                    async move {
                        counter.fetch_add(1, Ordering::SeqCst);
                        match response {
                            Some(body) => Ok(axum::Json(body)),
                            None => Err(axum::http::StatusCode::INTERNAL_SERVER_ERROR),
                        }
                    }
                }),
            );
        let mut heartbeat = Heartbeat::new(reconciler(serve(app).await, local_state()));
        heartbeat.failures = starting_failures;
        heartbeat.fence = guard;

        heartbeat.tick().await;

        let outcome = TickOutcome {
            failures: heartbeat.failures,
            heartbeats: heartbeats.load(Ordering::SeqCst),
        };
        (outcome, heartbeat.fence)
    }

    #[tokio::test]
    async fn successful_heartbeats_reset_failures() {
        let guard = fence::FenceGuard::new(0, Instant::now());
        let (outcome, _) = run_tick(Some(serde_json::json!({})), guard, 2).await;

        assert_eq!(outcome.heartbeats, 1);
        assert_eq!(outcome.failures, 0);
    }

    #[tokio::test]
    async fn failed_heartbeats_are_counted() {
        let guard = fence::FenceGuard::new(0, Instant::now());
        let (outcome, _) = run_tick(None, guard, 1).await;

        assert_eq!(outcome.heartbeats, 1);
        assert_eq!(outcome.failures, 2);
    }

    #[tokio::test]
    async fn rejected_heartbeats_never_fence_the_agent() {
        let long_ago = Instant::now() - Duration::from_secs(3600);
        let guard = fence::FenceGuard::new(1, long_ago);

        let (outcome, mut guard) = run_tick(None, guard, 0).await;

        assert_eq!(outcome.failures, 1);
        assert!(!guard.record_isolation(long_ago + Duration::from_secs(1)));
    }

    #[test]
    fn heartbeat_backoff_is_bounded() {
        let period = Duration::from_secs(30);
        assert_eq!(next_tick_delay(period, 0), period);
        for failures in 1..50 {
            let delay = next_tick_delay(period, failures);
            assert!(delay >= period && delay <= HEARTBEAT_BACKOFF_CAP);
        }
    }

    #[test]
    fn loopback_gateways_do_not_adopt_advertised_addresses() {
        adopt_advertised_gateway("http://127.0.0.1:8000", Some("https://other"));
        adopt_advertised_gateway("https://same", Some("https://same"));
        adopt_advertised_gateway("https://gw", None);
    }
}
