use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tracing::warn;

use super::Reconciler;
use crate::state::Phase;
use crate::{client, local_state, spec};

const READINESS_TIMEOUT: Duration = Duration::from_secs(1);
const READINESS_GIVE_UP: Duration = Duration::from_secs(300);
const STATS_CONCURRENCY: usize = 16;

type ReadinessProbes = HashMap<String, (String, Vec<u16>)>;

static PROBING_SINCE: std::sync::LazyLock<std::sync::Mutex<HashMap<String, Instant>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(HashMap::new()));

impl Reconciler {
    pub async fn container_statuses(&self) -> Vec<client::ContainerStatus> {
        let probes = readiness_probes(&self.local_state).await;
        let running = self.workloads.running().await;
        let mut statuses = Vec::with_capacity(running.len());

        for (workload_id, kind, container_id) in running {
            let runtime = self.runtimes.get(kind);
            let mut status = match runtime.inspect_status(&container_id).await {
                Ok(status) => status,
                Err(e) => {
                    warn!(workload_id = %workload_id, container_id = %container_id, error = %e, "Failed to inspect workload");
                    "failed".to_string()
                }
            };
            if status == "running" && !is_ready(&probes, &workload_id).await {
                status = "starting".to_string();
            }
            if status != "creating" {
                self.workloads.clear_phase(&workload_id).await;
            }
            statuses.push(client::ContainerStatus {
                workload_id,
                container_id,
                status,
            });
        }

        statuses.extend(
            self.workloads
                .pending()
                .await
                .into_iter()
                .map(pending_status),
        );
        statuses
    }

    pub async fn push_workload_stats(&self) {
        let running = self.workloads.running().await;
        if running.is_empty() {
            return;
        }

        let stats: Vec<client::WorkloadStatsUpdate> = futures_util::stream::iter(running)
            .map(|(workload_id, kind, container_id)| async move {
                match self.runtimes.get(kind).stats(&container_id).await {
                    Ok(stats) => Some(client::WorkloadStatsUpdate {
                        workload_id,
                        cpu_usage_percent: stats.cpu_usage_percent,
                        memory_usage_bytes: stats.memory_usage_bytes,
                        network_rx_bytes: stats.network_rx_bytes,
                        network_tx_bytes: stats.network_tx_bytes,
                        volume_usage: stats.volume_usage,
                    }),
                    Err(e) => {
                        warn!(workload_id = %workload_id, container_id = %container_id, error = %e, "Failed to collect container stats");
                        None
                    }
                }
            })
            .buffer_unordered(STATS_CONCURRENCY)
            .filter_map(std::future::ready)
            .collect()
            .await;

        if let Err(e) = self.client.push_workload_stats(&self.api_key, stats).await {
            warn!(error = %e, "Failed to push workload stats");
        }
    }
}

fn pending_status((workload_id, phase): (String, Phase)) -> client::ContainerStatus {
    client::ContainerStatus {
        workload_id,
        container_id: String::new(),
        status: phase.as_str().to_string(),
    }
}

async fn is_ready(probes: &ReadinessProbes, workload_id: &str) -> bool {
    let Some((address, ports)) = probes.get(workload_id) else {
        return true;
    };
    accepts_connections(address, ports).await || probe_expired(workload_id)
}

async fn readiness_probes(local_state: &local_state::LocalState) -> ReadinessProbes {
    let workloads = local_state.workloads().await.unwrap_or_default();
    let probes: ReadinessProbes = workloads
        .into_iter()
        .filter(|workload| workload.replaces_workload_id.is_some())
        .filter_map(|workload| {
            let ports: Vec<u16> = workload
                .ports
                .as_ref()?
                .iter()
                .filter(|port| port.protocol.unwrap_or_default() == spec::PortProtocol::Tcp)
                .map(|port| port.container_port)
                .collect();
            (!ports.is_empty()).then_some(())?;
            Some((workload.id, (workload.guest_ip?, ports)))
        })
        .collect();
    PROBING_SINCE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .retain(|id, _| probes.contains_key(id));
    probes
}

fn probe_expired(workload_id: &str) -> bool {
    PROBING_SINCE
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .entry(workload_id.to_string())
        .or_insert_with(Instant::now)
        .elapsed()
        >= READINESS_GIVE_UP
}

async fn accepts_connections(address: &str, ports: &[u16]) -> bool {
    for port in ports {
        let attempt = tokio::time::timeout(
            READINESS_TIMEOUT,
            tokio::net::TcpStream::connect((address, *port)),
        )
        .await;
        if matches!(attempt, Ok(Ok(_))) {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{json_client, local_state, reconciler};
    use super::*;
    use crate::state::Kind;

    #[tokio::test]
    async fn readiness_accepts_any_listening_port_and_rejects_closed_ones() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let open = listener.local_addr().unwrap().port();
        let closed = {
            let probe = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            probe.local_addr().unwrap().port()
        };
        assert!(accepts_connections("127.0.0.1", &[closed, open]).await);
        assert!(!accepts_connections("127.0.0.1", &[closed]).await);
        assert!(!probe_expired("fresh-workload"));
    }

    #[tokio::test]
    async fn container_statuses_include_pending_phases() {
        let _tools = crate::testing::FakeTools::install(&[("ip", "exit 0")]);
        let reconciler = reconciler(json_client(&[]).await, local_state());
        reconciler
            .workloads
            .started("a", Kind::Microvm, "ca".to_string())
            .await;
        reconciler.workloads.set_phase("a", Phase::Creating).await;
        reconciler
            .workloads
            .started("b", Kind::Vm, "cb".to_string())
            .await;
        reconciler
            .workloads
            .set_phase("pending", Phase::Pulling)
            .await;

        let mut statuses = reconciler.container_statuses().await;
        statuses.sort_by(|left, right| left.workload_id.cmp(&right.workload_id));

        assert_eq!(statuses.len(), 3);
        assert_eq!(statuses[0].status, "failed");
        assert_eq!(statuses[1].status, "failed");
        assert_eq!(statuses[2].workload_id, "pending");
        assert_eq!(statuses[2].status, "pulling");
        assert!(statuses[2].container_id.is_empty());
        assert_eq!(reconciler.workloads.pending().await.len(), 1);
    }

    #[tokio::test]
    async fn workload_stats_are_pushed_only_when_something_runs() {
        let _tools = crate::testing::FakeTools::install(&[("ip", "exit 0")]);
        let app = axum::Router::new().route(
            "/api/agents/self/workloads/stats",
            axum::routing::post(|| async { axum::http::StatusCode::OK }),
        );
        let reconciler = reconciler(super::super::fixtures::serve(app).await, local_state());
        reconciler.push_workload_stats().await;
        reconciler
            .workloads
            .started("w", Kind::Microvm, "c".to_string())
            .await;
        reconciler
            .workloads
            .started("v", Kind::Vm, "d".to_string())
            .await;
        reconciler.push_workload_stats().await;
    }
}
