use super::*;

const HELD: Option<Duration> = Some(Duration::from_secs(15));

#[test]
fn a_stable_running_replacement_takes_over() {
    assert_eq!(
        next_step(true, true, WorkloadStatus::Running, 30, HELD),
        Step::Switch
    );
}

#[test]
fn a_young_or_starting_replacement_waits() {
    let brief = Some(Duration::from_secs(2));
    assert_eq!(
        next_step(true, true, WorkloadStatus::Running, 30, brief),
        Step::Wait
    );
    assert_eq!(
        next_step(true, true, WorkloadStatus::Starting, 30, None),
        Step::Wait
    );
}

#[test]
fn failed_stale_or_orphaned_replacements_are_abandoned() {
    assert_eq!(
        next_step(true, true, WorkloadStatus::Failed, 30, None),
        Step::Abandon
    );
    assert_eq!(
        next_step(true, true, WorkloadStatus::Starting, 700, None),
        Step::Abandon
    );
    assert_eq!(
        next_step(false, true, WorkloadStatus::Running, 30, HELD),
        Step::Abandon
    );
    assert_eq!(
        next_step(true, false, WorkloadStatus::Running, 30, HELD),
        Step::Abandon
    );
}

#[test]
fn replacements_with_a_changed_image_are_not_a_rotation() {
    let base = sample_workload();
    let mut changed = base.clone();
    changed.image = "other:2".to_string();
    assert!(same_spec(&base, &base.clone()));
    assert!(!same_spec(&base, &changed));
}

fn sample_workload() -> workloads::Model {
    workloads::Model {
        id: Uuid::new_v4(),
        name: "w".to_string(),
        image: "nginx".to_string(),
        cpu_millicores: 1,
        memory_bytes: 1,
        disk_bytes: 0,
        cpu_limit_millicores: None,
        memory_limit_bytes: None,
        spread_key: None,
        guest_ip: None,
        auto_resources: false,
        cpu_usage_peak_percent: None,
        memory_usage_peak_bytes: None,
        env_vars: None,
        secret_versions: None,
        replaces_workload_id: None,
        retire_after: None,
        rollout_error: None,
        ports: None,
        volume_mounts: None,
        status: "running".to_string(),
        assigned_agent_id: None,
        container_id: None,
        created_by: None,
        organization_id: None,
        resource_group_id: None,
        stack_id: None,
        service_name: None,
        restart_policy: "always".to_string(),
        max_restarts: None,
        restart_count: 0,
        runtime_class: "firecracker".to_string(),
        desired_state: "running".to_string(),
        restart_requested: false,
        cpu_usage_percent: None,
        memory_usage_bytes: None,
        network_rx_bytes: None,
        network_tx_bytes: None,
        volume_usage: None,
        stats_updated_at: None,
        created_at: Utc::now().naive_utc(),
        updated_at: None,
        site_id: String::new(),
        zone: String::new(),
        generation: 0,
        observed_generation: 0,
        observed_status: None,
        topology_spread: "none".to_string(),
        vm_config: None,
    }
}

#[test]
fn the_toggle_accepts_common_spellings_of_off() {
    assert!(surge_enabled(None));
    for off in ["false", "0", "no", "OFF"] {
        assert!(!surge_enabled(Some(off)));
    }
}

#[test]
fn only_spec_changes_trigger_a_replacement() {
    let mut update = UpdateWorkloadRequest {
        max_restarts: Some(3),
        ..Default::default()
    };
    assert!(!touches_spec(&update));
    update.image = Some("nginx:2".to_string());
    assert!(touches_spec(&update));
}
