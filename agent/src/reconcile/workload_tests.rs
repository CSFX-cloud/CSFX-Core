use anyhow::Result;
use tokio::sync::Mutex;

use super::fixtures::{json_client, local_state, reconciler, unreachable_client, workload};
use super::Reconciler;
use crate::client;
use crate::runtime::{ContainerStats, ExecSession, LogStream, Runtime};
use crate::spec::WorkloadSpec;
use crate::state::{Kind, Phase, WorkloadEntry};

struct ScriptedRuntime {
    status: Option<&'static str>,
    pull_fails: bool,
    start_fails: bool,
    stop_fails: bool,
    started: Mutex<Vec<WorkloadSpec>>,
    stopped: Mutex<Vec<String>>,
}

impl ScriptedRuntime {
    fn new() -> Self {
        Self {
            status: Some("running"),
            pull_fails: false,
            start_fails: false,
            stop_fails: false,
            started: Mutex::new(Vec::new()),
            stopped: Mutex::new(Vec::new()),
        }
    }

    fn with_status(status: Option<&'static str>) -> Self {
        Self {
            status,
            ..Self::new()
        }
    }
}

#[async_trait::async_trait]
impl Runtime for ScriptedRuntime {
    async fn pull_image(&self, _image: &str) -> Result<()> {
        if self.pull_fails {
            anyhow::bail!("registry unreachable");
        }
        Ok(())
    }

    async fn start_workload(&self, spec: &WorkloadSpec) -> Result<String> {
        if self.start_fails {
            anyhow::bail!("boot failed");
        }
        self.started.lock().await.push(spec.clone());
        Ok(format!("handle-{}", spec.workload_id))
    }

    async fn inspect_status(&self, _workload_handle: &str) -> Result<String> {
        self.status
            .map(str::to_string)
            .ok_or_else(|| anyhow::anyhow!("inspect failed"))
    }

    fn logs(&self, _workload_handle: &str) -> LogStream {
        Box::pin(futures_util::stream::empty())
    }

    async fn exec(&self, _workload_handle: &str) -> Result<ExecSession> {
        anyhow::bail!("unsupported")
    }

    async fn stop_workload(&self, workload_handle: &str) -> Result<()> {
        if self.stop_fails {
            anyhow::bail!("stop refused");
        }
        self.stopped.lock().await.push(workload_handle.to_string());
        Ok(())
    }

    async fn stats(&self, _workload_handle: &str) -> Result<ContainerStats> {
        Ok(ContainerStats::default())
    }
}

async fn fixture() -> Reconciler {
    reconciler(json_client(&[]).await, local_state())
}

fn workload_with(extra: serde_json::Value) -> client::AssignedWorkload {
    let mut base = serde_json::json!({
        "id": "w1", "image": "img", "cpu_millicores": 200, "memory_bytes": 1024,
        "restart_policy": "always", "disk_bytes": 4096
    });
    base.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    serde_json::from_value(base).unwrap()
}

fn workload_json(id: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id, "image": "img", "cpu_millicores": 100, "memory_bytes": 1,
        "env_vars": null, "ports": null, "volume_mounts": null, "service_name": null,
        "restart_policy": "always", "max_restarts": null, "resource_group_id": null,
        "resource_group_cidr": null, "runtime_class": "microvm", "disk_bytes": 0
    })
}

#[tokio::test]
async fn healthy_and_unreachable_containers_are_not_restarted() {
    let state = fixture().await;
    let wl = workload("w", "always", None);
    let running = ScriptedRuntime::with_status(Some("running"));
    assert!(!state.should_restart_after_crash(&running, &wl, "c").await);
    let broken = ScriptedRuntime::with_status(None);
    assert!(!state.should_restart_after_crash(&broken, &wl, "c").await);
}

#[tokio::test]
async fn crashed_containers_follow_the_restart_policy() {
    let state = fixture().await;
    let failed = ScriptedRuntime::with_status(Some("failed"));

    let never = workload("n", "never", None);
    assert!(
        !state
            .should_restart_after_crash(&failed, &never, "cn")
            .await
    );
    assert_eq!(*failed.stopped.lock().await, vec!["cn".to_string()]);

    let limited = workload("l", "always", Some(1));
    assert!(
        state
            .should_restart_after_crash(&failed, &limited, "cl")
            .await
    );
    assert!(
        !state
            .should_restart_after_crash(&failed, &limited, "cl")
            .await
    );
    assert_eq!(failed.stopped.lock().await.len(), 2);

    let unlimited = workload("u", "always", None);
    assert!(
        state
            .should_restart_after_crash(&failed, &unlimited, "cu")
            .await
    );
    assert!(
        state
            .should_restart_after_crash(&failed, &unlimited, "cu")
            .await
    );
    assert_eq!(state.workloads.restarts("u").await, 2);
}

#[tokio::test]
async fn tear_down_stops_the_container_and_forgets_the_workload() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();
    state
        .workloads
        .started("gone", Kind::Vm, "handle-gone".to_string())
        .await;
    let entry = WorkloadEntry {
        kind: Kind::Vm,
        handle: Some("handle-gone".to_string()),
        ..WorkloadEntry::default()
    };

    state.tear_down(&runtime, "gone", &entry).await;

    assert_eq!(*runtime.stopped.lock().await, vec!["handle-gone"]);
    assert!(state.workloads.running().await.is_empty());
}

#[tokio::test]
async fn failed_tear_downs_keep_the_workload_tracked() {
    let state = fixture().await;
    let mut runtime = ScriptedRuntime::new();
    runtime.stop_fails = true;
    state
        .workloads
        .started("gone", Kind::Microvm, "handle-gone".to_string())
        .await;
    let entry = WorkloadEntry {
        handle: Some("handle-gone".to_string()),
        ..WorkloadEntry::default()
    };

    state.tear_down(&runtime, "gone", &entry).await;

    assert_eq!(state.workloads.running().await.len(), 1);
}

#[tokio::test]
async fn removed_workloads_are_dropped_and_assigned_ones_kept() {
    let _tools = crate::testing::FakeTools::install(&[("ip", "exit 0")]);
    let state = fixture().await;
    for id in ["keep", "container-gone", "vm-gone"] {
        let kind = if id == "vm-gone" {
            Kind::Vm
        } else {
            Kind::Microvm
        };
        state
            .workloads
            .started(id, kind, format!("handle-{id}"))
            .await;
    }
    state
        .workloads
        .set_phase("pending-gone", Phase::Pulling)
        .await;

    state.reap_stale(&[workload("keep", "always", None)]).await;

    let running = state.workloads.running().await;
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].0, "keep");
    assert!(state.workloads.pending().await.is_empty());
}

#[tokio::test]
async fn started_workloads_are_tracked_and_report_the_starting_phase() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();

    let restarted = state
        .start_or_restart(&runtime, workload_with(serde_json::json!({})))
        .await;

    assert!(!restarted);
    assert_eq!(
        state.workloads.running().await,
        vec![("w1".to_string(), Kind::Microvm, "handle-w1".to_string())]
    );
    let spec = runtime.started.lock().await[0].clone();
    assert_eq!(spec.cpu_limit_millicores, 200);
    assert_eq!(spec.memory_limit_bytes, 1024);
    assert!(spec.vm_spec.is_none());
}

#[tokio::test]
async fn limits_never_drop_below_the_requests() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();
    let workload = workload_with(serde_json::json!({
        "cpu_limit_millicores": 50, "memory_limit_bytes": 10
    }));

    state.start_or_restart(&runtime, workload).await;

    let spec = runtime.started.lock().await[0].clone();
    assert_eq!(
        (spec.cpu_limit_millicores, spec.memory_limit_bytes),
        (200, 1024)
    );
}

#[tokio::test]
async fn virtual_machines_receive_a_boot_disk_specification() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();

    state
        .start_or_restart(
            &runtime,
            workload_with(serde_json::json!({"runtime_class": "vm", "image": "http://iso/os.iso"})),
        )
        .await;

    let vm = runtime.started.lock().await[0].vm_spec.clone().unwrap();
    assert_eq!(vm.boot_disk_size_bytes, 4096);
    assert_eq!(vm.iso_url.as_deref(), Some("http://iso/os.iso"));
    assert!(vm.boot_disk_path.contains("w1"));
    assert_eq!(state.workloads.running().await[0].1, Kind::Vm);
}

#[tokio::test]
async fn workloads_wait_until_their_volumes_are_mounted() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();
    let workload = workload_with(serde_json::json!({
        "volume_mounts": [{"volume_id": "vol-1", "mount_path": "/data"}]
    }));

    assert!(!state.start_or_restart(&runtime, workload.clone()).await);
    assert_eq!(
        state.workloads.pending().await,
        vec![("w1".to_string(), Phase::Scheduled)]
    );
    assert!(runtime.started.lock().await.is_empty());

    state
        .volumes
        .lock()
        .await
        .insert("vol-1".to_string(), "/dev/rbd0".to_string());
    state.start_or_restart(&runtime, workload).await;
    let spec = runtime.started.lock().await[0].clone();
    let mounts = spec.volume_mounts.unwrap();
    assert_eq!(mounts[0].device_path, "/dev/rbd0");
    assert_eq!(mounts[0].mount_path, "/data");
}

#[tokio::test]
async fn image_pull_failures_clear_the_phase() {
    let state = fixture().await;
    let mut runtime = ScriptedRuntime::new();
    runtime.pull_fails = true;

    assert!(
        !state
            .start_or_restart(&runtime, workload_with(serde_json::json!({})))
            .await
    );

    assert!(state.workloads.pending().await.is_empty());
    assert!(state.workloads.running().await.is_empty());
}

#[tokio::test]
async fn exhausted_restart_budgets_fail_the_workload() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();
    for _ in 0..3 {
        state.workloads.record_restart("w1", None).await.unwrap();
    }

    state
        .start_or_restart(
            &runtime,
            workload_with(serde_json::json!({"max_restarts": 3})),
        )
        .await;

    assert_eq!(
        state.workloads.pending().await,
        vec![("w1".to_string(), Phase::Failed)]
    );
    assert!(runtime.started.lock().await.is_empty());
}

#[tokio::test]
async fn start_failures_are_counted_and_marked_failed() {
    let state = fixture().await;
    let mut runtime = ScriptedRuntime::new();
    runtime.start_fails = true;

    for _ in 0..2 {
        assert!(
            !state
                .start_or_restart(&runtime, workload_with(serde_json::json!({})))
                .await
        );
    }

    assert_eq!(
        state.workloads.pending().await,
        vec![("w1".to_string(), Phase::Failed)]
    );
    assert_eq!(state.workloads.restarts("w1").await, 2);
}

#[tokio::test]
async fn requested_restarts_stop_the_old_container_first() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();
    state
        .workloads
        .started("w1", Kind::Microvm, "old-handle".to_string())
        .await;

    let restarted = state
        .start_or_restart(
            &runtime,
            workload_with(serde_json::json!({"restart_requested": true})),
        )
        .await;

    assert!(restarted);
    assert_eq!(
        *runtime.stopped.lock().await,
        vec!["old-handle".to_string()]
    );
    assert_eq!(
        state.workloads.handle("w1").await.as_deref(),
        Some("handle-w1")
    );
}

#[tokio::test]
async fn healthy_running_workloads_are_left_alone() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();
    state
        .workloads
        .started("w1", Kind::Microvm, "handle".to_string())
        .await;

    assert!(
        !state
            .start_or_restart(&runtime, workload_with(serde_json::json!({})))
            .await
    );

    assert!(runtime.started.lock().await.is_empty());
    assert!(runtime.stopped.lock().await.is_empty());
}

#[tokio::test]
async fn failed_stops_abort_the_restart() {
    let state = fixture().await;
    let mut runtime = ScriptedRuntime::new();
    runtime.stop_fails = true;
    state
        .workloads
        .started("w1", Kind::Microvm, "old-handle".to_string())
        .await;

    let restarted = state
        .start_or_restart(
            &runtime,
            workload_with(serde_json::json!({"restart_requested": true})),
        )
        .await;

    assert!(!restarted);
    assert_eq!(
        state.workloads.handle("w1").await.as_deref(),
        Some("old-handle")
    );
    assert!(runtime.started.lock().await.is_empty());
}

#[tokio::test]
async fn unresolved_secrets_leave_the_workload_untouched() {
    let state = fixture().await;
    let runtime = ScriptedRuntime::new();

    assert!(
        !state
            .start_or_restart(
                &runtime,
                workload_with(serde_json::json!({"env_unresolved": true})),
            )
            .await
    );

    assert!(runtime.started.lock().await.is_empty());
    assert!(state.workloads.pending().await.is_empty());
}

#[tokio::test]
async fn fetched_workloads_are_written_to_local_state() {
    let client = json_client(&[(
        "/api/agents/self/workloads",
        serde_json::json!([workload_json("w1")]),
    )])
    .await;
    let state = reconciler(client, local_state());

    let workloads = state.desired_workloads().await.unwrap();

    assert_eq!(workloads.len(), 1);
    assert_eq!(state.local_state.workloads().await.unwrap()[0].id, "w1");
}

#[tokio::test]
async fn an_unreachable_control_plane_falls_back_to_local_state() {
    let state = reconciler(unreachable_client(), local_state());
    state
        .local_state
        .confirm_workloads(&[workload("w1", "always", None)])
        .await;

    let workloads = state.desired_workloads().await.unwrap();

    assert_eq!(workloads.len(), 1);
    assert_eq!(workloads[0].id, "w1");
}

#[tokio::test]
async fn without_local_state_an_unreachable_control_plane_yields_nothing() {
    let state = reconciler(unreachable_client(), local_state());

    assert!(state.desired_workloads().await.is_none());
}

#[tokio::test]
async fn an_empty_confirmed_list_replaces_the_local_state() {
    let client = json_client(&[("/api/agents/self/workloads", serde_json::json!([]))]).await;
    let state = reconciler(client, local_state());
    state
        .local_state
        .confirm_workloads(&[workload("w1", "always", None)])
        .await;

    let workloads = state.desired_workloads().await.unwrap();

    assert!(workloads.is_empty());
    assert!(state.local_state.workloads().await.unwrap().is_empty());
}

#[tokio::test]
async fn a_full_reconcile_pass_against_an_empty_assignment_is_idle() {
    let _tools = crate::testing::FakeTools::install(&[
        ("wg", "exit 0"),
        ("ip", "exit 0"),
        ("nft", "exit 0"),
    ]);
    let empty = serde_json::json!([]);
    let client = json_client(&[
        ("/api/agents/self/volumes", empty.clone()),
        ("/api/agents/self/workloads", empty.clone()),
        ("/api/agents/mesh-peers", empty.clone()),
        ("/api/resource-groups/agent/active-ids", empty),
    ])
    .await;
    let state = reconciler(client, local_state());

    state.reconcile().await;

    assert!(state.workloads.running().await.is_empty());
}
