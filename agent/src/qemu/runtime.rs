use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::cgroup_stats::{
    read_cpu_usage_percent, read_memory_working_set_bytes, service_cgroup_path,
};
use crate::qemu::machine::{data_disk_args, firmware_args, nic_device_args, Architecture};
use crate::qemu::qmp::QmpClient;
use crate::rg_dhcp::RgDhcpSupervisor;
use crate::spec::WorkloadSpec;
use crate::vm_host::{self, process_gid, process_uid};
use nics::{read_tap_bytes, remove_taps, tap_devices, GuestNic};

const VM_STATE_DIR: &str = "/var/lib/csfx-agent/qemu";
const QMP_SOCKET_NAME: &str = "qmp.sock";
const VNC_SOCKET_NAME: &str = "vnc.sock";
const BYTES_PER_MIB: i64 = 1024 * 1024;
const MIN_MEMORY_MIB: i64 = 512;
const HOST_MEMORY_OVERHEAD_MIB: i64 = 1024;

mod nics;

fn qemu_bin_path() -> String {
    std::env::var("CSFX_QEMU_BIN_PATH")
        .unwrap_or_else(|_| Architecture::HOST.default_qemu_bin_path().to_string())
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
    qmp_socket_path: PathBuf,
    vnc_socket_path: PathBuf,
    resource_group_id: Option<String>,
    extra_nic_count: usize,
    previous_cpu_sample: Arc<Mutex<Option<(std::time::Instant, u64)>>>,
}

pub struct QemuRuntime {
    wg_private_key_b64: String,
    local_state: Arc<crate::local_state::LocalState>,
    rg_dns_registry: Arc<crate::rg_dns::RgDnsRegistry>,
    dhcp_supervisor: RgDhcpSupervisor,
    handles: Mutex<HashMap<String, VmHandle>>,
    reconciled: AtomicBool,
}

impl QemuRuntime {
    pub fn new(
        wg_private_key_b64: String,
        local_state: Arc<crate::local_state::LocalState>,
        rg_dns_registry: Arc<crate::rg_dns::RgDnsRegistry>,
    ) -> Self {
        Self {
            wg_private_key_b64,
            local_state,
            rg_dns_registry,
            dhcp_supervisor: RgDhcpSupervisor::new(),
            handles: Mutex::new(HashMap::new()),
            reconciled: AtomicBool::new(false),
        }
    }

    pub async fn qmp_socket_path(&self, workload_id: &str) -> Option<PathBuf> {
        self.handles
            .lock()
            .await
            .get(workload_id)
            .map(|h| h.qmp_socket_path.clone())
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
        vm_host::recover_once(self, &self.reconciled, "vm").await
    }

    async fn ensure_rg_network(
        &self,
        resource_group_id: &str,
        resource_group_cidr: Option<&str>,
    ) -> Result<String> {
        crate::rg_network::ensure_group(
            &self.local_state,
            resource_group_id,
            resource_group_cidr,
            &self.wg_private_key_b64,
            &self.rg_dns_registry,
        )
        .await
    }

    pub async fn teardown_rg_network(&self, resource_group_id: &str) -> Result<()> {
        self.dhcp_supervisor.stop(resource_group_id).await;
        crate::rg_network::teardown_group(&self.local_state, resource_group_id).await
    }
}

#[async_trait::async_trait]
impl crate::runtime::Runtime for QemuRuntime {
    async fn start_workload(&self, spec: &WorkloadSpec) -> Result<String> {
        let vm_spec = spec
            .vm_spec
            .as_ref()
            .context("qemu runtime requires a vm_spec")?;

        let dir = vm_dir(&spec.workload_id);
        tokio::fs::create_dir_all(&dir)
            .await
            .context("Failed to create vm state directory")?;

        vm_host::write_sidecar(
            &dir,
            &VmSidecarMetadata {
                resource_group_id: spec.resource_group_id.clone(),
                extra_nic_count: vm_spec.config.extra_nics.len(),
            },
        )
        .await
        .context("Failed to write vm sidecar metadata")?;

        ensure_architecture(&vm_spec.config)?;
        ensure_boot_disk(&vm_spec.boot_disk_path, vm_spec.boot_disk_size_bytes).await?;
        let data_disks = stage_data_disks(&spec.workload_id, spec, &dir).await?;

        let iso_path = match &vm_spec.iso_url {
            Some(iso_url) => {
                let path = ensure_iso_cached(&spec.workload_id, iso_url).await?;
                ensure_raw_image(&path).await?;
                Some(path)
            }
            None => None,
        };

        let bridge_iface = match &spec.resource_group_id {
            Some(resource_group_id) => Some(
                self.ensure_rg_network(resource_group_id, spec.resource_group_cidr.as_deref())
                    .await?,
            ),
            None => None,
        };

        let nics = self
            .attach_networks(spec, &vm_spec.config.extra_nics, bridge_iface.as_deref())
            .await?;

        let qmp_socket_path = dir.join(QMP_SOCKET_NAME);
        let vnc_socket_path = dir.join(VNC_SOCKET_NAME);

        let boot_result = spawn_vm(
            &spec.workload_id,
            spec,
            vm_spec,
            iso_path.as_deref(),
            &data_disks,
            &nics,
            &qmp_socket_path,
            &vnc_socket_path,
        )
        .await;

        if let Err(e) = boot_result {
            stop_unit(&spec.workload_id).await;
            let tap_devices: Vec<String> = nics.iter().map(|nic| nic.tap_device.clone()).collect();
            remove_taps(&spec.workload_id, &tap_devices).await;
            return Err(e);
        }

        self.handles.lock().await.insert(
            spec.workload_id.clone(),
            VmHandle {
                qmp_socket_path,
                vnc_socket_path,
                resource_group_id: spec.resource_group_id.clone(),
                extra_nic_count: vm_spec.config.extra_nics.len(),
                previous_cpu_sample: Arc::default(),
            },
        );

        info!(workload_id = %spec.workload_id, guest_ip = ?nics[0].ip, nic_count = nics.len(), "QEMU VM started");

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

        if let Some(status) = query_vm_status(&qmp_socket_path).await {
            return Ok(status);
        }
        if is_unit_active(&unit_name(workload_handle)).await {
            warn!(workload_id = %workload_handle, "QMP did not answer but the qemu unit is active, keeping the vm");
            return Ok("running".to_string());
        }
        Ok("failed".to_string())
    }

    fn logs(&self, _workload_handle: &str) -> crate::runtime::LogStream {
        Box::pin(futures_util::stream::empty())
    }

    async fn exec(&self, _workload_handle: &str) -> Result<crate::runtime::ExecSession> {
        anyhow::bail!("exec is not supported for qemu vm workloads, use the vnc console instead")
    }

    async fn stop_workload(&self, workload_handle: &str) -> Result<()> {
        let handle = self.handles.lock().await.remove(workload_handle);

        stop_unit(workload_handle).await;

        let dir = vm_dir(workload_handle);
        let sidecar = vm_host::read_sidecar::<VmSidecarMetadata>(&dir).await;
        let extra_nic_count = sidecar
            .as_ref()
            .map_or(0, |sidecar| sidecar.extra_nic_count);
        remove_taps(
            workload_handle,
            &tap_devices(workload_handle, extra_nic_count),
        )
        .await;

        let resource_group_id = match handle {
            Some(handle) => handle.resource_group_id,
            None => sidecar.and_then(|sidecar| sidecar.resource_group_id),
        };

        if let Some(resource_group_id) = &resource_group_id {
            self.dhcp_supervisor
                .remove_reservation(resource_group_id, workload_handle)
                .await;
            crate::rg_ipam::release(resource_group_id, workload_handle).await;
        }

        let _ = tokio::fs::remove_file(dir.join(QMP_SOCKET_NAME)).await;
        let _ = tokio::fs::remove_file(dir.join(VNC_SOCKET_NAME)).await;
        let _ = tokio::fs::remove_file(dir.join("qemu.pid")).await;

        info!(workload_id = %workload_handle, "QEMU VM stopped");

        Ok(())
    }

    async fn stats(&self, workload_handle: &str) -> Result<crate::runtime::ContainerStats> {
        let (previous_cpu_sample, extra_nic_count) = {
            let handles = self.handles.lock().await;
            let Some(handle) = handles.get(workload_handle) else {
                return Ok(crate::runtime::ContainerStats::default());
            };
            (
                Arc::clone(&handle.previous_cpu_sample),
                handle.extra_nic_count,
            )
        };
        let cgroup_path = service_cgroup_path(&unit_name(workload_handle));
        let (network_rx_bytes, network_tx_bytes) =
            read_tap_bytes(&tap_devices(workload_handle, extra_nic_count))
                .await
                .unzip();

        Ok(crate::runtime::ContainerStats {
            cpu_usage_percent: read_cpu_usage_percent(&cgroup_path, &previous_cpu_sample).await,
            memory_usage_bytes: read_memory_working_set_bytes(&cgroup_path).await,
            network_rx_bytes,
            network_tx_bytes,
            volume_usage: Vec::new(),
        })
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

            let sidecar = vm_host::read_sidecar::<VmSidecarMetadata>(&dir).await;

            let handle = VmHandle {
                qmp_socket_path: dir.join(QMP_SOCKET_NAME),
                vnc_socket_path: dir.join(VNC_SOCKET_NAME),
                extra_nic_count: sidecar.as_ref().map_or(0, |s| s.extra_nic_count),
                resource_group_id: sidecar.and_then(|s| s.resource_group_id),
                previous_cpu_sample: Arc::default(),
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

async fn stop_unit(workload_id: &str) {
    let unit = unit_name(workload_id);
    match Command::new("systemctl")
        .args(["stop", &unit])
        .output()
        .await
    {
        Ok(output) if output.status.success() => {}
        Ok(output) => warn!(
            unit = %unit,
            stderr = %String::from_utf8_lossy(&output.stderr).trim(),
            "Failed to stop qemu unit"
        ),
        Err(e) => warn!(unit = %unit, error = %e, "Failed to execute systemctl stop"),
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
    #[serde(default)]
    extra_nic_count: usize,
}

fn iso_download_client() -> Result<reqwest::Client> {
    let ca_pem = crate::pki::AgentPki::load_ca_pem().context("Failed to load cluster CA")?;
    let ca = reqwest::Certificate::from_pem(ca_pem.as_bytes()).context("Invalid cluster CA")?;
    reqwest::Client::builder()
        .tls_certs_merge([ca])
        .build()
        .context("Failed to build iso download client")
}

async fn ensure_iso_cached(workload_id: &str, iso_url: &str) -> Result<PathBuf> {
    let path = iso_cache_path(workload_id);
    if tokio::fs::metadata(&path).await.is_ok() {
        return Ok(path);
    }

    info!(workload_id = %workload_id, "Downloading vm iso image");

    let client = iso_download_client()?;

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

async fn ensure_raw_image(path: &Path) -> Result<()> {
    let output = Command::new("qemu-img")
        .args(["info", "--output=json"])
        .arg(path)
        .output()
        .await
        .context("Failed to execute qemu-img info")?;

    if !output.status.success() {
        anyhow::bail!("qemu-img info failed for iso {}", path.display());
    }

    let info: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("Failed to parse qemu-img info output")?;
    match info["format"].as_str() {
        Some("raw") => Ok(()),
        format => anyhow::bail!(
            "iso {} has unsupported format {}",
            path.display(),
            format.unwrap_or("unknown")
        ),
    }
}

fn ensure_architecture(config: &crate::spec::VmConfig) -> Result<()> {
    let Some(requested) = config.architecture.as_deref() else {
        return Ok(());
    };
    anyhow::ensure!(
        Architecture::from_name(requested) == Some(Architecture::HOST),
        "vm requires {requested} but this node is {}",
        Architecture::HOST.name()
    );
    Ok(())
}

async fn stage_data_disks(
    workload_id: &str,
    spec: &WorkloadSpec,
    dir: &Path,
) -> Result<Vec<PathBuf>> {
    let mut staged = Vec::new();
    let unit_prefix = unit_name(workload_id);
    for mount in spec.volume_mounts.as_deref().unwrap_or_default() {
        let name = format!("data-{}", mount.volume_id);
        vm_host::stage_block_device(&mount.device_path, &unit_prefix, dir, &name)
            .await
            .context("Failed to stage volume block device for vm")?;
        staged.push(dir.join(name));
    }
    Ok(staged)
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

#[allow(clippy::too_many_arguments)]
async fn spawn_vm(
    workload_id: &str,
    spec: &WorkloadSpec,
    vm_spec: &crate::spec::VmSpec,
    iso_path: Option<&Path>,
    data_disks: &[PathBuf],
    nics: &[GuestNic],
    qmp_socket_path: &Path,
    vnc_socket_path: &Path,
) -> Result<()> {
    let vcpu_count = spec.vcpu_count();
    let mem_size_mib = (spec.memory_limit_bytes / BYTES_PER_MIB).max(MIN_MEMORY_MIB);
    let qemu_bin = qemu_bin_path();

    let architecture = Architecture::HOST;
    let mut args: Vec<String> = vec![
        "-sandbox".to_string(),
        "on,obsolete=deny,elevateprivileges=deny,spawn=deny,resourcecontrol=deny".to_string(),
    ];
    args.extend(architecture.machine_args());
    args.extend([
        "-smp".to_string(),
        vcpu_count.to_string(),
        "-m".to_string(),
        mem_size_mib.to_string(),
        "-drive".to_string(),
        format!("file={},format=raw,if=virtio", vm_spec.boot_disk_path),
        "-display".to_string(),
        "none".to_string(),
    ]);
    args.extend(architecture.display_args());
    for (index, nic) in nics.iter().enumerate() {
        args.extend([
            "-netdev".to_string(),
            format!(
                "tap,id=net{index},ifname={},script=no,downscript=no",
                nic.tap_device
            ),
        ]);
        args.extend(nic_device_args(
            vm_spec.config.nic_model.as_deref(),
            &nic.mac_address,
            index,
        ));
    }
    args.extend([
        "-device".to_string(),
        "virtio-balloon-pci,deflate-on-oom=on".to_string(),
        "-qmp".to_string(),
        format!("unix:{},server,nowait", qmp_socket_path.display()),
        "-vnc".to_string(),
        format!("unix:{}", vnc_socket_path.display()),
    ]);
    for (index, path) in data_disks.iter().enumerate() {
        args.extend(data_disk_args(index, path));
    }
    args.extend(firmware_args(architecture, &vm_dir(workload_id)).await?);

    if let Some(iso_path) = iso_path {
        args.extend(architecture.iso_drive_args(iso_path));
    }

    debug!(workload_id = %workload_id, args = ?args, stage = "spawn_vm", "Launching qemu");

    let uid_arg = format!("--uid={}", process_uid());
    let gid_arg = format!("--gid={}", process_gid());
    let cpu_weight_arg = format!("--property=CPUWeight={}", spec.cpu_weight());
    let cpu_quota_arg = format!("--property=CPUQuota={}%", spec.cpu_quota_percent());
    let memory_max_arg = format!(
        "--property=MemoryMax={}M",
        mem_size_mib + HOST_MEMORY_OVERHEAD_MIB
    );

    let status = Command::new("systemd-run")
        .args([
            "--unit",
            &unit_name(workload_id),
            "--collect",
            &uid_arg,
            &gid_arg,
            &cpu_weight_arg,
            &cpu_quota_arg,
            &memory_max_arg,
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

    wait_for_qmp(qmp_socket_path).await?;
    set_balloon_target(qmp_socket_path, spec, mem_size_mib).await
}

async fn set_balloon_target(
    qmp_socket_path: &Path,
    spec: &WorkloadSpec,
    mem_size_mib: i64,
) -> Result<()> {
    if spec.balloon_bytes() == 0 {
        return Ok(());
    }

    let target_bytes =
        (mem_size_mib * BYTES_PER_MIB - spec.balloon_bytes()).max(MIN_MEMORY_MIB * BYTES_PER_MIB);
    let socket_str = qmp_socket_path.display().to_string();
    let mut client = QmpClient::connect(&socket_str)
        .await
        .context("Failed to connect to QMP for balloon")?;
    client
        .send_command(
            "balloon",
            Some(serde_json::json!({ "value": target_bytes })),
        )
        .await
        .context("Failed to set balloon target")?;
    Ok(())
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

#[cfg(test)]
mod tests;
