use super::*;

const GIB: i64 = 1024 * 1024 * 1024;

fn resources(cpu: Option<i32>, memory: Option<i64>, disk: Option<i64>) -> UpdateWorkloadRequest {
    UpdateWorkloadRequest {
        cpu_millicores: cpu,
        memory_bytes: memory,
        disk_bytes: disk,
        ..Default::default()
    }
}

fn with_architecture(architecture: &str) -> UpdateWorkloadRequest {
    UpdateWorkloadRequest {
        vm_config: Some(VmConfig {
            architecture: Some(architecture.to_string()),
            ..Default::default()
        }),
        ..Default::default()
    }
}

#[test]
fn whole_vcpus_and_enough_memory_are_accepted() {
    assert!(validate_resources(
        20 * GIB,
        &resources(Some(4000), Some(4 * GIB), Some(40 * GIB))
    )
    .is_ok());
    assert!(validate_resources(20 * GIB, &resources(None, None, None)).is_ok());
}

#[test]
fn fractional_or_missing_vcpus_are_rejected() {
    assert!(validate_resources(GIB, &resources(Some(1500), None, None)).is_err());
    assert!(validate_resources(GIB, &resources(Some(0), None, None)).is_err());
}

#[test]
fn memory_below_the_floor_is_rejected() {
    assert!(validate_resources(GIB, &resources(None, Some(GIB / 4), None)).is_err());
}

#[test]
fn the_disk_can_grow_but_not_shrink() {
    assert!(validate_resources(20 * GIB, &resources(None, None, Some(20 * GIB))).is_ok());
    assert!(validate_resources(20 * GIB, &resources(None, None, Some(10 * GIB))).is_err());
}

#[test]
fn the_architecture_cannot_change() {
    assert!(validate_config(Some("x86_64"), &with_architecture("x86_64")).is_ok());
    assert!(validate_config(Some("x86_64"), &with_architecture("aarch64")).is_err());
    assert!(validate_config(Some("x86_64"), &UpdateWorkloadRequest::default()).is_ok());
}

#[test]
fn capacity_counts_only_the_growth_beyond_the_current_size() {
    assert!(!exceeds(2, 4, Some(6)));
    assert!(exceeds(1, 4, Some(6)));
    assert!(!exceeds(0, 4, Some(3)));
    assert!(!exceeds(0, 4, None));
}
