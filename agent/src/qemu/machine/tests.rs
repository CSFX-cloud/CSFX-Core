use super::*;
use std::os::unix::fs::PermissionsExt;

#[test]
fn x86_iso_is_attached_as_a_readonly_raw_cdrom() {
    let args = Architecture::X86_64.iso_drive_args(Path::new("/vm/install.iso"));

    assert_eq!(args[0], "-drive");
    assert_eq!(
        args[1],
        "file=/vm/install.iso,media=cdrom,format=raw,readonly=on,if=ide"
    );
    assert!(!args.contains(&"-cdrom".to_string()));
}

#[test]
fn aarch64_iso_is_a_bootable_scsi_cdrom() {
    let args = Architecture::Aarch64.iso_drive_args(Path::new("/vm/install.iso"));

    assert!(args[1].contains("media=cdrom") && args[1].contains("readonly=on"));
    assert!(args[1].contains("if=none,id=install-iso"));
    assert!(args.contains(&"scsi-cd,drive=install-iso,bootindex=0".to_string()));
    assert!(!args.iter().any(|arg| arg.contains("if=ide")));
}

#[test]
fn machine_types_match_the_architecture() {
    assert!(Architecture::X86_64
        .machine_args()
        .contains(&"q35".to_string()));
    assert!(Architecture::Aarch64
        .machine_args()
        .contains(&"virt,gic-version=max".to_string()));
    assert!(!Architecture::Aarch64
        .display_args()
        .contains(&"-vga".to_string()));
}

#[tokio::test]
async fn x86_needs_no_firmware() {
    let args = firmware_args(Architecture::X86_64, Path::new("/nonexistent"))
        .await
        .unwrap();

    assert!(args.is_empty());
}

#[tokio::test]
async fn aarch64_firmware_gets_a_private_variable_store() {
    let dir = std::env::temp_dir().join(format!("csfx-fw-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let code = dir.join("code.fd");
    let vars = dir.join("vars.fd");
    std::fs::write(&code, b"code").unwrap();
    std::fs::write(&vars, b"vars").unwrap();
    std::fs::set_permissions(&vars, std::fs::Permissions::from_mode(0o444)).unwrap();
    std::env::set_var(FIRMWARE_CODE_ENV, &code);
    std::env::set_var(FIRMWARE_VARS_ENV, &vars);

    let vm_dir = dir.join("vm");
    std::fs::create_dir_all(&vm_dir).unwrap();
    let args = firmware_args(Architecture::Aarch64, &vm_dir).await.unwrap();

    let copied = vm_dir.join(FIRMWARE_VARS_FILE);
    assert_eq!(std::fs::read(&copied).unwrap(), b"vars");
    assert!(!std::fs::metadata(&copied).unwrap().permissions().readonly());
    assert!(args[1].contains("readonly=on"));
    assert!(args[3].contains(FIRMWARE_VARS_FILE));
    std::env::remove_var(FIRMWARE_CODE_ENV);
    std::env::remove_var(FIRMWARE_VARS_ENV);
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn architectures_round_trip_through_their_names() {
    for architecture in [Architecture::X86_64, Architecture::Aarch64] {
        assert_eq!(
            Architecture::from_name(architecture.name()),
            Some(architecture)
        );
    }
    assert_eq!(Architecture::from_name("riscv64"), None);
}

#[test]
fn nic_models_map_to_qemu_devices() {
    assert_eq!(
        nic_device_args(None, "52:54:00:00:00:01")[1],
        "virtio-net-pci,netdev=net0,mac=52:54:00:00:00:01"
    );
    assert_eq!(
        nic_device_args(Some("e1000e"), "52:54:00:00:00:01")[1],
        "e1000e,netdev=net0,mac=52:54:00:00:00:01"
    );
    assert_eq!(
        nic_device_args(Some("unknown"), "m")[1],
        "virtio-net-pci,netdev=net0,mac=m"
    );
}

#[test]
fn data_disks_are_virtio_block_devices() {
    let args = data_disk_args(1, Path::new("/vm/data-1"));

    assert!(args[1].starts_with("file=/vm/data-1,format=raw,if=none,id=data1"));
    assert_eq!(args[3], "virtio-blk-pci,drive=data1");
}
