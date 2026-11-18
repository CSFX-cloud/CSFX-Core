mod network;
mod status;
mod volumes;
#[cfg(test)]
mod workload_tests;
mod workloads;

use std::sync::Arc;
use tracing::error;

use crate::client::ApiClient;
use crate::firecracker::runtime::FirecrackerRuntime;
use crate::local_state::LocalState;
use crate::qemu::runtime::QemuRuntime;
use crate::rg_dns::RgDnsRegistry;
use crate::runtime::Runtime;
use crate::start_tracker::StartTracker;
use crate::state::{Kind, WorkloadTable};
use crate::{rg_mesh, rg_network};

pub use volumes::MountedVolumes;

#[derive(Clone)]
pub struct Runtimes {
    pub firecracker: Arc<FirecrackerRuntime>,
    pub qemu: Arc<QemuRuntime>,
}

impl Runtimes {
    pub fn get(&self, kind: Kind) -> &dyn Runtime {
        match kind {
            Kind::Vm => self.qemu.as_ref(),
            Kind::Microvm => self.firecracker.as_ref(),
        }
    }
}

#[derive(Clone)]
pub struct Reconciler {
    pub client: Arc<ApiClient>,
    pub api_key: Arc<str>,
    pub agent_id: uuid::Uuid,
    pub runtimes: Runtimes,
    pub workloads: WorkloadTable,
    pub volumes: MountedVolumes,
    pub starts: StartTracker,
    pub rg_dns: Arc<RgDnsRegistry>,
    pub local_state: Arc<LocalState>,
}

impl Reconciler {
    pub async fn reconcile(&self) {
        let assigned_volumes = self.process_volumes().await;
        let resource_group_ids = self.process_workloads().await;
        if let Some(volumes) = &assigned_volumes {
            self.release_unassigned_volumes(volumes).await;
        }
        if let Err(e) = rg_network::reconcile_bridges().await {
            error!(error = %e, "Failed to reconcile resource group bridges");
        }
        rg_mesh::sync(
            &self.client,
            &self.api_key,
            self.agent_id,
            &resource_group_ids,
            &self.rg_dns,
            &self.local_state,
        )
        .await;
        self.sync_mgmt_peers(&resource_group_ids).await;
        self.runtimes.firecracker.check_dns_liveness().await;
        self.runtimes.qemu.check_dhcp_liveness().await;
        self.cleanup_stale_resource_groups().await;
    }

    pub async fn fence_all(&self) {
        self.reap_stale(&[]).await;
        self.release_unassigned_volumes(&[]).await;
        self.local_state.clear().await;
    }
}

#[cfg(test)]
pub(crate) mod fixtures {
    use super::*;
    use axum::routing::get;
    use axum::Router;

    pub fn local_state() -> LocalState {
        let dir = std::env::temp_dir().join(format!("csfx-agent-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        LocalState::open(dir.join("desired-state.json"))
    }

    pub fn runtimes() -> Runtimes {
        crate::client::install_test_crypto_provider();
        let registry = Arc::new(RgDnsRegistry::new());
        let state = Arc::new(local_state());
        Runtimes {
            firecracker: Arc::new(FirecrackerRuntime::new(
                "k".to_string(),
                state.clone(),
                registry.clone(),
                None,
            )),
            qemu: Arc::new(QemuRuntime::new("k".to_string(), state, registry)),
        }
    }

    pub fn reconciler(client: ApiClient, local_state: LocalState) -> Reconciler {
        Reconciler {
            client: Arc::new(client),
            api_key: Arc::from("k"),
            agent_id: uuid::Uuid::nil(),
            runtimes: runtimes(),
            workloads: WorkloadTable::default(),
            volumes: MountedVolumes::default(),
            starts: StartTracker::default(),
            rg_dns: Arc::new(RgDnsRegistry::new()),
            local_state: Arc::new(local_state),
        }
    }

    pub async fn serve(app: Router) -> ApiClient {
        crate::client::install_test_crypto_provider();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        ApiClient::new(url, "pub".to_string(), None, None, None).unwrap()
    }

    pub async fn json_client(routes: &[(&'static str, serde_json::Value)]) -> ApiClient {
        let mut app = Router::new();
        for (path, body) in routes {
            let body = body.clone();
            app = app.route(
                path,
                get(move || {
                    let body = body.clone();
                    async move { axum::Json(body) }
                }),
            );
        }
        serve(app).await
    }

    pub fn unreachable_client() -> ApiClient {
        crate::client::install_test_crypto_provider();
        ApiClient::new(
            "http://127.0.0.1:1".to_string(),
            "pub".to_string(),
            None,
            None,
            None,
        )
        .unwrap()
    }

    pub fn workload(
        id: &str,
        policy: &str,
        max_restarts: Option<i32>,
    ) -> crate::client::AssignedWorkload {
        serde_json::from_value(serde_json::json!({
            "id": id, "image": "img", "cpu_millicores": 100, "memory_bytes": 1,
            "restart_policy": policy, "max_restarts": max_restarts, "disk_bytes": 1
        }))
        .unwrap()
    }
}
