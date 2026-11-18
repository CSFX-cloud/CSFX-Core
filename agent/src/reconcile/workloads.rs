use std::collections::{HashMap, HashSet};
use tracing::{info, warn};

use super::Reconciler;
use crate::runtime::Runtime;
use crate::state::{Kind, Phase, WorkloadEntry};
use crate::{client, isolation, qemu, spec};

impl Reconciler {
    pub(super) async fn process_workloads(&self) -> Vec<String> {
        let Some(workloads) = self.desired_workloads().await else {
            self.adopt_running(&[]).await;
            return Vec::new();
        };

        self.prepare_networks(&workloads).await;
        self.adopt_running(&workloads).await;
        self.reap_stale(&workloads).await;

        let resource_group_ids = resource_group_ids(&workloads);
        self.spawn_starts(workloads).await;
        resource_group_ids
    }

    pub(super) async fn desired_workloads(&self) -> Option<Vec<client::AssignedWorkload>> {
        let error = match self.client.fetch_assigned_workloads(&self.api_key).await {
            Ok(workloads) => {
                self.local_state.confirm_workloads(&workloads).await;
                return Some(workloads);
            }
            Err(e) => e,
        };
        warn!(error = %error, "Failed to fetch assigned workloads");
        if isolation::isolated_from_peers().await {
            warn!(
                "Control plane and all mesh peers unreachable, not acting on local workload state"
            );
            return None;
        }
        let workloads = self.local_state.workloads().await?;
        info!(
            generation = self.local_state.generation().await,
            workloads = workloads.len(),
            "Reconciling from local workload state"
        );
        Some(workloads)
    }

    async fn adopt_running(&self, workloads: &[client::AssignedWorkload]) {
        let microvms = self.runtimes.firecracker.reconcile_once().await;
        let vms = self.runtimes.qemu.reconcile_once().await;

        self.workloads
            .mark_kinds(
                workloads
                    .iter()
                    .map(|w| (w.id.clone(), Kind::of(&w.runtime_class))),
            )
            .await;
        self.workloads
            .adopt(tagged(microvms, Kind::Microvm).chain(tagged(vms, Kind::Vm)))
            .await;
    }

    pub(super) async fn reap_stale(&self, desired: &[client::AssignedWorkload]) {
        let desired_ids: HashSet<&str> = desired.iter().map(|w| w.id.as_str()).collect();

        for (workload_id, entry) in self.workloads.stale(&desired_ids).await {
            let runtime = self.runtimes.get(entry.kind);
            self.tear_down(runtime, &workload_id, &entry).await;
        }
    }

    pub(super) async fn tear_down(
        &self,
        runtime: &dyn Runtime,
        workload_id: &str,
        entry: &WorkloadEntry,
    ) {
        if let Some(container_id) = &entry.handle {
            info!(workload_id = %workload_id, container_id = %container_id, "Tearing down removed workload");
            if let Err(e) = runtime.stop_workload(container_id).await {
                warn!(workload_id = %workload_id, container_id = %container_id, error = %e, "Failed to tear down container");
                return;
            }
        }
        self.workloads.remove(workload_id).await;
    }

    async fn spawn_starts(&self, workloads: Vec<client::AssignedWorkload>) {
        self.starts.forget_unassigned(&workloads).await;
        for workload in workloads {
            if self.starts.try_begin(&workload.id).await {
                tokio::spawn(self.clone().run_start(workload));
            }
        }
    }

    async fn run_start(self, workload: client::AssignedWorkload) {
        let workload_id = workload.id.clone();
        let _permit = self.starts.permit().await;
        let runtime = self.runtimes.get(Kind::of(&workload.runtime_class));
        let handle_before = self.workloads.handle(&workload_id).await;
        let deferred = !self.volumes_ready(&workload).await;

        let restart_fulfilled = self.start_or_restart(runtime, workload).await;
        if restart_fulfilled {
            if let Err(e) = self
                .client
                .ack_workload_restart(&self.api_key, &workload_id)
                .await
            {
                warn!(workload_id = %workload_id, error = %e, "Failed to ack workload restart");
            }
        }

        let handle_after = self.workloads.handle(&workload_id).await;
        let state_changed =
            restart_fulfilled || (handle_after.is_some() && handle_after != handle_before);
        self.starts
            .finish(
                &workload_id,
                state_changed,
                handle_after.is_none() && !deferred,
            )
            .await;
    }

    pub(super) async fn start_or_restart(
        &self,
        runtime: &dyn Runtime,
        workload: client::AssignedWorkload,
    ) -> bool {
        if workload.env_unresolved {
            warn!(workload_id = %workload.id, "Secrets could not be resolved, leaving the workload as it is");
            return false;
        }
        if let Some(container_id) = self.workloads.handle(&workload.id).await {
            if !self
                .restart_existing(runtime, &workload, &container_id)
                .await
            {
                return false;
            }
        }

        info!(workload_id = %workload.id, image = %workload.image, "Starting workload");

        if !self.volumes_ready(&workload).await {
            info!(workload_id = %workload.id, "Waiting for volumes to be mounted, deferring workload");
            self.workloads
                .set_phase(&workload.id, Phase::Scheduled)
                .await;
            return false;
        }
        self.launch(runtime, workload).await
    }

    async fn restart_existing(
        &self,
        runtime: &dyn Runtime,
        workload: &client::AssignedWorkload,
        container_id: &str,
    ) -> bool {
        let should_restart = workload.restart_requested
            || self
                .should_restart_after_crash(runtime, workload, container_id)
                .await;
        if !should_restart {
            return false;
        }
        if let Err(e) = runtime.stop_workload(container_id).await {
            warn!(workload_id = %workload.id, container_id = %container_id, error = %e, "Failed to stop container for restart");
            return false;
        }
        self.workloads.clear_handle(&workload.id).await;
        true
    }

    async fn launch(&self, runtime: &dyn Runtime, workload: client::AssignedWorkload) -> bool {
        let workload_id = workload.id.clone();
        if self.restart_budget_exhausted(&workload).await {
            self.workloads.set_phase(&workload_id, Phase::Failed).await;
            return false;
        }

        self.workloads
            .set_phase(&workload_id, Phase::Creating)
            .await;
        let kind = Kind::of(&workload.runtime_class);
        let restart_requested = workload.restart_requested;
        let devices = self.volumes.lock().await.clone();

        match runtime
            .start_workload(&build_spec(workload, &devices))
            .await
        {
            Ok(container_id) => {
                info!(workload_id = %workload_id, container_id = %container_id, "Workload started");
                self.workloads
                    .started(&workload_id, kind, container_id)
                    .await;
                restart_requested
            }
            Err(e) => {
                let attempt = self
                    .workloads
                    .record_restart(&workload_id, None)
                    .await
                    .unwrap_or_else(|count| count);
                warn!(workload_id = %workload_id, error = ?e, attempt, "Failed to start workload");
                self.workloads.set_phase(&workload_id, Phase::Failed).await;
                false
            }
        }
    }

    async fn restart_budget_exhausted(&self, workload: &client::AssignedWorkload) -> bool {
        let Some(max) = workload.max_restarts else {
            return false;
        };
        self.workloads.restarts(&workload.id).await as i32 >= max
    }

    pub(super) async fn should_restart_after_crash(
        &self,
        runtime: &dyn Runtime,
        workload: &client::AssignedWorkload,
        container_id: &str,
    ) -> bool {
        let status = match runtime.inspect_status(container_id).await {
            Ok(status) => status,
            Err(e) => {
                warn!(workload_id = %workload.id, container_id = %container_id, error = %e, "Failed to inspect container");
                return false;
            }
        };

        if status != "failed" {
            return false;
        }

        if workload.restart_policy == "never" {
            stop_logged(runtime, workload, container_id, "restart_policy=never").await;
            return false;
        }

        match self
            .workloads
            .record_restart(&workload.id, workload.max_restarts)
            .await
        {
            Ok(count) => {
                info!(workload_id = %workload.id, restart_count = count, "Container crashed, scheduling restart");
                true
            }
            Err(count) => {
                warn!(workload_id = %workload.id, restart_count = count, max_restarts = ?workload.max_restarts, "Crash-loop limit reached, stopping container");
                stop_logged(runtime, workload, container_id, "crash-loop limit").await;
                false
            }
        }
    }
}

async fn stop_logged(
    runtime: &dyn Runtime,
    workload: &client::AssignedWorkload,
    container_id: &str,
    reason: &str,
) {
    if let Err(e) = runtime.stop_workload(container_id).await {
        warn!(workload_id = %workload.id, container_id = %container_id, reason, error = %e, "Failed to stop container");
    }
}

fn tagged(
    recovered: Vec<(String, String)>,
    kind: Kind,
) -> impl Iterator<Item = (String, Kind, String)> {
    recovered
        .into_iter()
        .map(move |(workload_id, handle)| (workload_id, kind, handle))
}

fn resource_group_ids(workloads: &[client::AssignedWorkload]) -> Vec<String> {
    let mut ids: Vec<String> = workloads
        .iter()
        .filter_map(|w| w.resource_group_id.clone())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn build_spec(
    workload: client::AssignedWorkload,
    devices: &HashMap<String, String>,
) -> spec::WorkloadSpec {
    let vm_spec = (workload.runtime_class == "vm").then(|| spec::VmSpec {
        boot_disk_path: qemu::runtime::boot_disk_path(&workload.id),
        boot_disk_size_bytes: workload.disk_bytes,
        iso_url: Some(workload.image.clone()),
    });

    spec::WorkloadSpec {
        workload_id: workload.id.clone(),
        image: workload.image.clone(),
        cpu_millicores: workload.cpu_millicores,
        cpu_limit_millicores: workload
            .cpu_limit_millicores
            .unwrap_or(workload.cpu_millicores)
            .max(workload.cpu_millicores),
        memory_bytes: workload.memory_bytes,
        memory_limit_bytes: workload
            .memory_limit_bytes
            .unwrap_or(workload.memory_bytes)
            .max(workload.memory_bytes),
        env_vars: workload.env_vars,
        ports: workload.ports,
        volume_mounts: workload.volume_mounts.map(|mounts| {
            mounts
                .into_iter()
                .filter_map(|m| {
                    let device_path = devices.get(&m.volume_id)?.clone();
                    Some(spec::VolumeMount {
                        volume_id: m.volume_id,
                        mount_path: m.mount_path,
                        device_path,
                    })
                })
                .collect()
        }),
        resource_group_id: workload.resource_group_id,
        resource_group_cidr: workload.resource_group_cidr,
        guest_ip: workload.guest_ip,
        vm_spec,
    }
}
