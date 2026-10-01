use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::qemu::qmp::QmpClient;
use crate::rg_dhcp::{mac_address_for_workload, RgDhcpSupervisor};
use crate::spec::WorkloadSpec;

const VM_STATE_DIR: &str = "/var/lib/csfx-agent/qemu";
const DEFAULT_QEMU_BIN_PATH: &str = "/usr/bin/qemu-system-x86_64";
const QMP_SOCKET_NAME: &str = "qmp.sock";
const VNC_SOCKET_NAME: &str = "vnc.sock";

fn qemu_bin_path() -> String {
    std::env::var("CSFX_QEMU_BIN_PATH").unwrap_or_else(|_| DEFAULT_QEMU_BIN_PATH.to_string())
}

fn process_uid() -> u32 {
    unsafe { libc::getuid() }
}

fn process_gid() -> u32 {
    unsafe { libc::getgid() }
}

fn vm_dir(workload_id: &str) -> PathBuf {
    Path::new(VM_STATE_DIR).join(workload_id)
}

fn iso_cache_path(workload_id: &str) -> PathBuf {
    vm_dir(workload_id).join("install.iso")
}

pub fn boot_disk_path(workload_id: &str) -> String {
    vm_dir(workload_id)
        .join("boot.img")
        .to_string_lossy()
        .to_string()
}

fn unit_name(workload_id: &str) -> String {
    format!("csfx-qemu-{}", short_id(workload_id))
}

const TAP_DEVICE_PREFIX: &str = "qtap";
const MAX_LINUX_IFNAME_LEN: usize = 15;

fn short_id(workload_id: &str) -> String {
    let max_len = MAX_LINUX_IFNAME_LEN - TAP_DEVICE_PREFIX.len();
    workload_id
        .chars()
        .filter(|c| *c != '-')
        .take(max_len)
        .collect()
}

struct VmHandle {
    workload_id: String,
    tap_device: String,
    qmp_socket_path: PathBuf,
    vnc_socket_path: PathBuf,
    resource_group_id: Option<String>,
}

pub struct QemuRuntime {
    wg_private_key_b64: String,
    rg_dns_registry: Arc<crate::rg_dns::RgDnsRegistry>,
    dhcp_supervisor: RgDhcpSupervisor,
    handles: Mutex<HashMap<String, VmHandle>>,
    reconciled: AtomicBool,
}

impl QemuRuntime {
    pub fn new(
        wg_private_key_b64: String,
        rg_dns_registry: Arc<crate::rg_dns::RgDnsRegistry>,
    ) -> Self {
        Self {
            wg_private_key_b64,
            rg_dns_registry,
            dhcp_supervisor: RgDhcpSupervisor::new(),
            handles: Mutex::new(HashMap::new()),
            reconciled: AtomicBool::new(false),
        }
    }

    pub async fn vnc_socket_path(&self, workload_id: &str) -> Option<PathBuf> {
        self.handles
            .lock()
            .await
            .get(workload_id)
            .map(|h| h.vnc_socket_path.clone())
    }

    pub async fn check_dhcp_liveness(&self) {
        self.dhcp_supervisor.check_liveness().await;
    }

    pub async fn reconcile_once(&self) -> Vec<(String, String)> {
        if self.reconciled.swap(true, Ordering::SeqCst) {
            return Vec::new();
        }

        match crate::runtime::Runtime::list_managed_workloads(self).await {
            Ok(recovered) => {
                info!(
                    count = recovered.len(),
                    "Reconciled running vms from disk state"
                );
                recovered
            }
            Err(e) => {
                warn!(error = %e, "Failed to reconcile qemu workloads from disk state");
                Vec::new()
            }
        }
    }

    async fn ensure_rg_network(
        &self,
        resource_group_id: &str,
        resource_group_cidr: Option<&str>,
    ) -> Result<String> {
        let iface = crate::rg_network::ensure_bridge(
            resource_group_id,
            resource_group_cidr,
            &self.rg_dns_registry,
        )
        .await
        .context("Failed to ensure resource group bridge")?;

        if let Some(cidr) = resource_group_cidr {
            let listen_port = crate::spec::rg_wireguard_port(resource_group_id);
            let wg_iface = crate::wireguard::ensure_interface(
                resource_group_id,
                &self.wg_private_key_b64,
                listen_port,
            )
            .await
            .context("Failed to bring up resource group WireGuard interface")?;

            crate::wireguard::set_route(&wg_iface, cidr)
                .await
                .context("Failed to route resource group CIDR over WireGuard interface")?;
        }

        Ok(iface)
    }

    pub async fn teardown_rg_network(&self, resource_group_id: &str) -> Result<()> {
        self.dhcp_supervisor.stop(resource_group_id).await;

        crate::rg_network::teardown_bridge(resource_group_id)
            .await
            .context("Failed to tear down resource group bridge")?;

        let wg_iface = crate::wireguard::rg_interface_name(resource_group_id);
        crate::wireguard::remove_interface(&wg_iface)
            .await
            .context("Failed to remove resource group WireGuard interface")?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::runtime::Runtime for QemuRuntime {
    async fn pull_image(&self, _image: &str) -> Result<()> {
        Ok(())
    }

    async fn start_workload(&self, spec: &WorkloadSpec) -> Result<String> {
        let vm_spec = spec
            .vm_spec
            .as_ref()
            .context("qemu runtime requires a vm_spec")?;

        let dir = vm_dir(&spec.workload_id);
        tokio::fs::create_dir_all(&dir)
            .await
            .context("Failed to create vm state directory")?;

        write_sidecar_metadata(
            &dir,
            &VmSidecarMetadata {
                resource_group_id: spec.resource_group_id.clone(),
            },
        )
        .await
        .context("Failed to write vm sidecar metadata")?;

        ensure_boot_disk(&vm_spec.boot_disk_path, vm_spec.boot_disk_size_bytes).await?;

        let iso_path = match &vm_spec.iso_url {
            Some(iso_url) => Some(ensure_iso_cached(&spec.workload_id, iso_url).await?),
            None => None,
        };

        let bridge_iface = match &spec.resource_group_id {
            Some(resource_group_id) => Some(
                self.ensure_rg_network(resource_group_id, spec.resource_group_cidr.as_deref())
                    .await?,
            ),
            None => None,
        };

        let guest_ip = match (&spec.resource_group_id, &spec.resource_group_cidr) {
            (Some(resource_group_id), Some(cidr)) => {
                Some(crate::rg_ipam::allocate(resource_group_id, cidr, &spec.workload_id).await?)
            }
            _ => None,
        };

        let mac_address = mac_address_for_workload(&spec.workload_id);

        if let (Some(resource_group_id), Some(cidr), Some(bridge_iface), Some(guest_ip)) = (
            &spec.resource_group_id,
            &spec.resource_group_cidr,
            &bridge_iface,
            &guest_ip,
        ) {
            self.dhcp_supervisor
                .add_reservation(
                    resource_group_id,
                    cidr,
                    bridge_iface,
                    &spec.workload_id,
                    &mac_address,
                    guest_ip,
                )
                .await
                .context("Failed to register resource group dhcp reservation")?;
        }

        let tap_device = format!("{}{}", TAP_DEVICE_PREFIX, short_id(&spec.workload_id));
        create_tap_device(&tap_device, bridge_iface.as_deref(), process_uid()).await?;

        let qmp_socket_path = dir.join(QMP_SOCKET_NAME);
        let vnc_socket_path = dir.join(VNC_SOCKET_NAME);

        let boot_result = spawn_vm(
            &spec.workload_id,
            spec,
            vm_spec,
            iso_path.as_deref(),
            &tap_device,
            &mac_address,
            &qmp_socket_path,
            &vnc_socket_path,
        )
        .await;

        if let Err(e) = boot_result {
            let _ = remove_tap_device(&tap_device).await;
            return Err(e);
        }

        self.handles.lock().await.insert(
            spec.workload_id.clone(),
            VmHandle {
                workload_id: spec.workload_id.clone(),
                tap_device,
                qmp_socket_path,
                vnc_socket_path,
                resource_group_id: spec.resource_group_id.clone(),
            },
        );

        info!(workload_id = %spec.workload_id, guest_ip = ?guest_ip, "QEMU VM started");

        Ok(spec.workload_id.clone())
    }

    async fn inspect_status(&self, workload_handle: &str) -> Result<String> {
        let qmp_socket_path = {
            let handles = self.handles.lock().await;
            let Some(handle) = handles.get(workload_handle) else {
                return Ok("failed".to_string());
            };
            handle.qmp_socket_path.clone()
        };

        let status = query_vm_status(&qmp_socket_path).await;
        Ok(status.unwrap_or_else(|| "failed".to_string()))
    }

    fn logs(&self, _workload_handle: &str) -> crate::runtime::LogStream {
        Box::pin(futures_util::stream::empty())
    }

    async fn exec(&self, _workload_handle: &str) -> Result<crate::runtime::ExecSession> {
        anyhow::bail!("exec is not supported for qemu vm workloads, use the vnc console instead")
    }

    async fn stop_workload(&self, workload_handle: &str) -> Result<()> {
        let mut handles = self.handles.lock().await;
        let Some(handle) = handles.remove(workload_handle) else {
            return Ok(());
        };

        let _ = Command::new("systemctl")
            .args(["stop", &unit_name(&handle.workload_id)])
            .status()
            .await;

        if let Err(e) = remove_tap_device(&handle.tap_device).await {
            warn!(workload_id = %handle.workload_id, error = %e, "Failed to remove tap device");
        }

        if let Some(resource_group_id) = &handle.resource_group_id {
            self.dhcp_supervisor
                .remove_reservation(resource_group_id, &handle.workload_id)
                .await;
            crate::rg_ipam::release(resource_group_id, &handle.workload_id).await;
        }

        let _ = tokio::fs::remove_file(&handle.qmp_socket_path).await;
        let _ = tokio::fs::remove_file(&handle.vnc_socket_path).await;
        let _ = tokio::fs::remove_file(vm_dir(&handle.workload_id).join("qemu.pid")).await;

        info!(workload_id = %handle.workload_id, "QEMU VM stopped");

        Ok(())
    }

    async fn stats(&self, _workload_handle: &str) -> Result<crate::runtime::ContainerStats> {
        Ok(crate::runtime::ContainerStats::default())
    }

    async fn list_managed_workloads(&self) -> Result<Vec<(String, String)>> {
        let mut entries = match tokio::fs::read_dir(VM_STATE_DIR).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).context("Failed to read vm state directory"),
        };

        let mut recovered = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .context("Failed to read vm state directory entry")?
        {
            let dir = entry.path();
            let Some(workload_id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };

            if !is_unit_active(&unit_name(&workload_id)).await {
                continue;
            }

            let sidecar = read_sidecar_metadata(&dir).await;

            let handle = VmHandle {
                workload_id: workload_id.clone(),
                tap_device: format!("{}{}", TAP_DEVICE_PREFIX, short_id(&workload_id)),
                qmp_socket_path: dir.join(QMP_SOCKET_NAME),
                vnc_socket_path: dir.join(VNC_SOCKET_NAME),
                resource_group_id: sidecar.and_then(|s| s.resource_group_id),
            };

            self.handles
                .lock()
                .await
                .insert(workload_id.clone(), handle);

            recovered.push((workload_id.clone(), workload_id));
        }

        Ok(recovered)
    }
}

async fn is_unit_active(unit: &str) -> bool {
    let output = Command::new("systemctl")
        .args(["is-active", unit])
        .output()
        .await;

    matches!(output, Ok(output) if output.stdout.starts_with(b"active"))
}

#[derive(serde::Serialize, serde::Deserialize)]
struct VmSidecarMetadata {
    resource_group_id: Option<String>,
}

fn sidecar_metadata_path(dir: &Path) -> PathBuf {
    dir.join("csfx-meta.json")
}

async fn write_sidecar_metadata(dir: &Path, metadata: &VmSidecarMetadata) -> Result<()> {
    let payload = serde_json::to_vec(metadata).context("Failed to serialize sidecar metadata")?;
    tokio::fs::write(sidecar_metadata_path(dir), payload)
        .await
        .context("Failed to write sidecar metadata file")?;
    Ok(())
}

async fn read_sidecar_metadata(dir: &Path) -> Option<VmSidecarMetadata> {
    let content = tokio::fs::read(sidecar_metadata_path(dir)).await.ok()?;
    serde_json::from_slice(&content).ok()
}

async fn ensure_iso_cached(workload_id: &str, iso_url: &str) -> Result<PathBuf> {
    let path = iso_cache_path(workload_id);
    if tokio::fs::metadata(&path).await.is_ok() {
        return Ok(path);
    }

    info!(workload_id = %workload_id, "Downloading vm iso image");

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()
        .context("Failed to build iso download client")?;

    let response = client
        .get(iso_url)
        .send()
        .await
        .context("Failed to request iso image")?
        .error_for_status()
        .context("Iso download request failed")?;

    let tmp_path = path.with_extension("iso.tmp");
    let mut file = tokio::fs::File::create(&tmp_path)
        .await
        .context("Failed to create iso temp file")?;

    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("Failed to read iso download chunk")?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
            .await
            .context("Failed to write iso chunk to disk")?;
    }

    tokio::fs::rename(&tmp_path, &path)
        .await
        .context("Failed to finalize iso image")?;

    info!(workload_id = %workload_id, path = ?path, "Vm iso image cached");

    Ok(path)
}

async fn ensure_boot_disk(path: &str, size_bytes: i64) -> Result<()> {
    if tokio::fs::metadata(path).await.is_ok() {
        return Ok(());
    }

    let status = Command::new("qemu-img")
        .args(["create", "-f", "raw", path, &size_bytes.to_string()])
        .status()
        .await
        .context("Failed to execute qemu-img create")?;

    if !status.success() {
        anyhow::bail!("qemu-img create failed for boot disk {}", path);
    }

    Ok(())
}

async fn create_tap_device(
    tap_device: &str,
    bridge_iface: Option<&str>,
    owner_uid: u32,
) -> Result<()> {
    let uid_arg = owner_uid.to_string();
    let output = Command::new("ip")
        .args([
            "tuntap", "add", "dev", tap_device, "mode", "tap", "user", &uid_arg,
        ])
        .output()
        .await
        .context("Failed to execute ip tuntap add")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.contains("File exists") {
            anyhow::bail!("failed to create tap device {}: {}", tap_device, stderr);
        }
    }

    if let Some(bridge) = bridge_iface {
        let status = Command::new("ip")
            .args(["link", "set", "dev", tap_device, "master", bridge])
            .status()
            .await
            .context("Failed to execute ip link set master")?;

        if !status.success() {
            anyhow::bail!(
                "failed to attach tap device {} to bridge {}",
                tap_device,
                bridge
            );
        }
    }

    let status = Command::new("ip")
        .args(["link", "set", "dev", tap_device, "up"])
        .status()
        .await
        .context("Failed to execute ip link set up")?;

    if !status.success() {
        anyhow::bail!("failed to bring up tap device {}", tap_device);
    }

    Ok(())
}

async fn remove_tap_device(tap_device: &str) -> Result<()> {
    let status = Command::new("ip")
        .args(["link", "delete", tap_device])
        .status()
        .await
        .context("Failed to execute ip link delete")?;

    if !status.success() {
        anyhow::bail!("failed to delete tap device {}", tap_device);
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn spawn_vm(
    workload_id: &str,
    spec: &WorkloadSpec,
    vm_spec: &crate::spec::VmSpec,
    iso_path: Option<&Path>,
    tap_device: &str,
    mac_address: &str,
    qmp_socket_path: &Path,
    vnc_socket_path: &Path,
) -> Result<()> {
    let vcpu_count = ((spec.cpu_millicores.max(100)) as f64 / 1000.0).ceil() as i64;
    let mem_size_mib = (spec.memory_bytes / 1024 / 1024).max(512);
    let qemu_bin = qemu_bin_path();

    let mut args: Vec<String> = vec![
        "-enable-kvm".to_string(),
        "-machine".to_string(),
        "q35".to_string(),
        "-cpu".to_string(),
        "host".to_string(),
        "-smp".to_string(),
        vcpu_count.to_string(),
        "-m".to_string(),
        mem_size_mib.to_string(),
        "-drive".to_string(),
        format!("file={},format=raw,if=virtio", vm_spec.boot_disk_path),
        "-display".to_string(),
        "none".to_string(),
        "-vga".to_string(),
        "std".to_string(),
        "-usb".to_string(),
        "-device".to_string(),
        "usb-tablet".to_string(),
        "-netdev".to_string(),
        format!("tap,id=net0,ifname={},script=no,downscript=no", tap_device),
        "-device".to_string(),
        format!("virtio-net-pci,netdev=net0,mac={}", mac_address),
        "-qmp".to_string(),
        format!("unix:{},server,nowait", qmp_socket_path.display()),
        "-vnc".to_string(),
        format!("unix:{}", vnc_socket_path.display()),
    ];

    if let Some(iso_path) = iso_path {
        args.push("-cdrom".to_string());
        args.push(iso_path.display().to_string());
        args.push("-boot".to_string());
        args.push("once=d".to_string());
    }

    debug!(workload_id = %workload_id, args = ?args, stage = "spawn_vm", "Launching qemu-system-x86_64");

    let uid_arg = format!("--uid={}", process_uid());
    let gid_arg = format!("--gid={}", process_gid());

    let status = Command::new("systemd-run")
        .args([
            "--unit",
            &unit_name(workload_id),
            "--collect",
            &uid_arg,
            &gid_arg,
            "--property=SupplementaryGroups=kvm",
            "--property=StandardOutput=journal",
            "--property=StandardError=journal",
            &qemu_bin,
        ])
        .args(&args)
        .status()
        .await
        .context("Failed to spawn qemu via systemd-run")?;

    if !status.success() {
        anyhow::bail!("systemd-run failed to start qemu unit for {}", workload_id);
    }

    wait_for_qmp(qmp_socket_path).await
}

async fn query_vm_status(qmp_socket_path: &Path) -> Option<String> {
    let socket_str = qmp_socket_path.display().to_string();
    let mut client = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        QmpClient::connect(&socket_str),
    )
    .await
    .ok()?
    .ok()?;

    let response = client.send_command("query-status", None).await.ok()?;
    let status = response.get("status")?.as_str()?;

    Some(match status {
        "guest-panicked" | "internal-error" | "io-error" | "shutdown" => "failed".to_string(),
        _ => "running".to_string(),
    })
}

async fn wait_for_qmp(qmp_socket_path: &Path) -> Result<()> {
    let socket_str = qmp_socket_path.display().to_string();
    let client = tokio::time::timeout(
        std::time::Duration::from_secs(10),
        QmpClient::connect(&socket_str),
    )
    .await
    .context("Timed out waiting for QMP socket")??;

    drop(client);
    Ok(())
}
