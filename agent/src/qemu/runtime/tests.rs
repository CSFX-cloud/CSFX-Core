use super::*;
use crate::runtime::Runtime;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

fn temp_dir() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("csfx-qemu-test-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn runtime() -> QemuRuntime {
    QemuRuntime::new(
        "key".to_string(),
        Arc::new(crate::local_state::LocalState::open(
            std::env::temp_dir().join(format!("csfx-ls-{}.json", uuid::Uuid::new_v4())),
        )),
        Arc::new(crate::rg_dns::RgDnsRegistry::new()),
    )
}

fn spec(memory_mib: i64, limit_mib: i64) -> WorkloadSpec {
    WorkloadSpec {
        workload_id: "w".to_string(),
        image: "i".to_string(),
        cpu_millicores: 1000,
        cpu_limit_millicores: 1000,
        memory_bytes: memory_mib * BYTES_PER_MIB,
        memory_limit_bytes: limit_mib * BYTES_PER_MIB,
        env_vars: None,
        ports: None,
        volume_mounts: None,
        resource_group_id: None,
        resource_group_cidr: None,
        guest_ip: None,
        vm_spec: None,
    }
}

async fn serve_qmp(path: &Path, replies: &'static [&'static str]) {
    let listener = UnixListener::bind(path).unwrap();
    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read_half, mut write_half) = stream.into_split();
        let mut reader = BufReader::new(read_half);
        write_half.write_all(b"{\"QMP\":{}}\n").await.unwrap();
        for reply in replies {
            let mut request = String::new();
            reader.read_line(&mut request).await.unwrap();
            write_half.write_all(reply.as_bytes()).await.unwrap();
        }
    });
}

fn handle(dir: &Path) -> VmHandle {
    VmHandle {
        qmp_socket_path: dir.join(QMP_SOCKET_NAME),
        vnc_socket_path: dir.join(VNC_SOCKET_NAME),
        resource_group_id: Some("rg".to_string()),
        extra_nic_count: 0,
        previous_cpu_sample: Arc::default(),
    }
}

#[test]
fn identifiers_fit_linux_interface_limits() {
    let id = "12345678-1234-1234-1234-123456789abc";
    assert_eq!(short_id(id), "12345678123");
    assert_eq!(format!("{TAP_DEVICE_PREFIX}{}", short_id(id)).len(), 15);
    assert_eq!(unit_name(id), "csfx-qemu-12345678123");
    assert!(boot_disk_path(id).ends_with(&format!("{id}/boot.img")));
    assert!(iso_cache_path(id).ends_with("install.iso"));
    assert!(!qemu_bin_path().is_empty());
}

#[tokio::test]
async fn vnc_socket_is_known_only_for_tracked_vms() {
    let runtime = runtime();
    let dir = temp_dir();
    assert!(runtime.vnc_socket_path("w").await.is_none());
    runtime
        .handles
        .lock()
        .await
        .insert("w".to_string(), handle(&dir));
    assert_eq!(
        runtime.vnc_socket_path("w").await,
        Some(dir.join(VNC_SOCKET_NAME))
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn status_is_derived_from_qmp() {
    let runtime = runtime();
    let dir = temp_dir();
    assert_eq!(runtime.inspect_status("w").await.unwrap(), "failed");
    serve_qmp(
        &dir.join(QMP_SOCKET_NAME),
        &[
            "{\"return\":{}}\n",
            "{\"return\":{\"status\":\"running\"}}\n",
        ],
    )
    .await;
    runtime
        .handles
        .lock()
        .await
        .insert("w".to_string(), handle(&dir));
    assert_eq!(runtime.inspect_status("w").await.unwrap(), "running");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn unanswered_qmp_only_fails_the_vm_when_its_unit_is_gone() {
    let dir = temp_dir();
    let runtime = runtime();
    runtime
        .handles
        .lock()
        .await
        .insert("w".to_string(), handle(&dir));
    {
        let _tools = FakeTools::install(&[("systemctl", "echo active")]);
        assert_eq!(runtime.inspect_status("w").await.unwrap(), "running");
    }
    let _tools = FakeTools::install(&[("systemctl", "echo inactive\nexit 3")]);
    assert_eq!(runtime.inspect_status("w").await.unwrap(), "failed");
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn panicked_guests_are_reported_as_failed() {
    let dir = temp_dir();
    let socket = dir.join(QMP_SOCKET_NAME);
    serve_qmp(
        &socket,
        &[
            "{\"return\":{}}\n",
            "{\"return\":{\"status\":\"guest-panicked\"}}\n",
        ],
    )
    .await;
    assert_eq!(query_vm_status(&socket).await.as_deref(), Some("failed"));
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn balloon_target_is_clamped_to_the_minimum_memory() {
    let dir = temp_dir();
    let socket = dir.join(QMP_SOCKET_NAME);
    set_balloon_target(&socket, &spec(512, 512), 512)
        .await
        .unwrap();
    serve_qmp(&socket, &["{\"return\":{}}\n", "{\"return\":{}}\n"]).await;
    set_balloon_target(&socket, &spec(256, 2048), 1024)
        .await
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

#[tokio::test]
async fn trivial_runtime_operations_have_defaults() {
    let runtime = runtime();
    runtime.check_dhcp_liveness().await;
    assert!(runtime.exec("w").await.is_err());
    let stats = runtime.stats("w").await.unwrap();
    assert!(stats.cpu_usage_percent.is_none());
    let dir = temp_dir();
    let existing = dir.join("boot.img");
    std::fs::write(&existing, b"x").unwrap();
    ensure_boot_disk(existing.to_str().unwrap(), 1)
        .await
        .unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
}

use crate::testing::FakeTools;

#[tokio::test]
async fn units_are_stopped_by_their_derived_name() {
    {
        let tools = FakeTools::install(&[("systemctl", "exit 0")]);
        stop_unit("0123456789abcdef").await;
        assert_eq!(
            tools.calls("systemctl"),
            vec![format!("stop {}", unit_name("0123456789abcdef"))]
        );
    }
    let tools = FakeTools::install(&[("systemctl", "echo 'not loaded' >&2\nexit 5")]);
    stop_unit("0123456789abcdef").await;
    assert_eq!(tools.calls("systemctl").len(), 1);
}

#[tokio::test]
async fn unit_activity_follows_the_reported_state() {
    {
        let _tools = FakeTools::install(&[("systemctl", "echo active")]);
        assert!(is_unit_active("csfx-qemu-x").await);
    }
    {
        let _tools = FakeTools::install(&[("systemctl", "echo inactive\nexit 3")]);
        assert!(!is_unit_active("csfx-qemu-x").await);
    }
    let _tools = FakeTools::install(&[]);
    std::env::set_var("PATH", "/nonexistent");
    assert!(!is_unit_active("csfx-qemu-x").await);
}

#[tokio::test]
async fn boot_disks_are_created_once() {
    let dir = temp_dir();
    let disk = dir.join("boot.raw");
    let disk = disk.to_str().unwrap();
    {
        let tools = FakeTools::install(&[("qemu-img", "exit 0")]);
        ensure_boot_disk(disk, 4096).await.unwrap();
        assert_eq!(
            tools.calls("qemu-img"),
            vec![format!("create -f raw {disk} 4096")]
        );
    }
    std::fs::File::create(disk).unwrap().set_len(4096).unwrap();
    let tools = FakeTools::install(&[("qemu-img", "exit 0")]);
    ensure_boot_disk(disk, 4096).await.unwrap();
    assert!(tools.calls("qemu-img").is_empty());
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn boot_disks_grow_when_the_workload_asks_for_more() {
    let dir = temp_dir();
    let disk = dir.join("boot.raw");
    let disk = disk.to_str().unwrap();
    std::fs::File::create(disk).unwrap().set_len(4096).unwrap();
    let tools = FakeTools::install(&[("qemu-img", "exit 0")]);

    ensure_boot_disk(disk, 8192).await.unwrap();

    assert_eq!(
        tools.calls("qemu-img"),
        vec![format!("resize -f raw {disk} 8192")]
    );
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn boot_disk_resize_failures_are_reported() {
    let dir = temp_dir();
    let disk = dir.join("boot.raw");
    std::fs::File::create(&disk).unwrap().set_len(4096).unwrap();
    let _tools = FakeTools::install(&[("qemu-img", "exit 1")]);

    let error = ensure_boot_disk(disk.to_str().unwrap(), 8192)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("qemu-img resize failed"));
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn boot_disk_creation_failures_are_reported() {
    let dir = temp_dir();
    let disk = dir.join("boot.raw");
    let _tools = FakeTools::install(&[("qemu-img", "exit 1")]);

    let error = ensure_boot_disk(disk.to_str().unwrap(), 4096)
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("qemu-img create failed"));
    std::fs::remove_dir_all(dir).ok();
}

#[tokio::test]
async fn raw_isos_are_accepted() {
    let _tools = FakeTools::install(&[("qemu-img", "echo '{\"format\": \"raw\"}'")]);

    ensure_raw_image(Path::new("/vm/install.iso"))
        .await
        .unwrap();
}

#[tokio::test]
async fn non_raw_isos_are_rejected() {
    for format in ["qcow2", "vmdk", "qed"] {
        let behaviour = format!("echo '{{\"format\": \"{format}\"}}'");
        let _tools = FakeTools::install(&[("qemu-img", behaviour.as_str())]);

        let error = ensure_raw_image(Path::new("/vm/install.iso"))
            .await
            .unwrap_err()
            .to_string();

        assert!(error.contains(&format!("unsupported format {format}")));
    }
}

#[tokio::test]
async fn unreadable_isos_are_rejected() {
    let _tools = FakeTools::install(&[("qemu-img", "exit 1")]);

    let error = ensure_raw_image(Path::new("/vm/install.iso"))
        .await
        .unwrap_err()
        .to_string();

    assert!(error.contains("qemu-img info failed"));
}
