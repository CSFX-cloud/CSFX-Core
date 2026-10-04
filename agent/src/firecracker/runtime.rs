use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::process::Command;
use tokio::sync::Mutex;
use tracing::{debug, info, warn};

use crate::firecracker::api::FirecrackerApiClient;
use crate::firecracker::rootfs::RootfsBuilder;
use crate::rg_dns_process::RgDnsProcessSupervisor;
use crate::runtime::{ExecSession, LogStream};
use crate::spec::{rg_wireguard_port, WorkloadSpec};

const JAILER_BASE_DIR: &str = "/var/lib/csfx-agent/firecracker";
const GUEST_KERNEL_PATH: &str = "/var/lib/csfx-agent/vmlinux";
const GUEST_CID_BASE: u32 = 1000;
const CGROUP_ROOT: &str = "/sys/fs/cgroup";
const CGROUP_PARENT: &str = "csfx-firecracker";
const DEFAULT_FIRECRACKER_BIN_PATH: &str = "/usr/bin/firecracker";
const API_SOCKET_NAME: &str = "firecracker.socket";
const METRICS_FILE_NAME: &str = "metrics.fifo";
const JAIL_KERNEL_NAME: &str = "vmlinux";
const JAIL_ROOTFS_NAME: &str = "rootfs.ext4";

fn firecracker_bin_path() -> String {
    std::env::var("CSFX_FIRECRACKER_BIN_PATH")
        .unwrap_or_else(|_| DEFAULT_FIRECRACKER_BIN_PATH.to_string())
}

fn process_uid() -> u32 {
    unsafe { libc::getuid() }
}

fn process_gid() -> u32 {
    unsafe { libc::getgid() }
}

fn next_free_cid_on_host() -> u32 {
    let output = std::process::Command::new("ip")
        .args(["tuntap", "show"])
        .output();

    let Ok(output) = output else {
        return GUEST_CID_BASE;
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let max_cid = stdout
        .lines()
        .filter_map(|line| line.split(':').next())
        .filter_map(|name| name.strip_prefix("fctap"))
        .filter_map(|cid| cid.parse::<u32>().ok())
        .max();

    match max_cid {
        Some(cid) => cid + 1,
        None => GUEST_CID_BASE,
    }
}

const JAILER_ID_LEN: usize = 8;

fn jailer_short_id(workload_id: &str) -> String {
    workload_id
        .chars()
        .filter(|c| *c != '-')
        .take(JAILER_ID_LEN)
        .collect()
}

fn jailer_chroot_base_dir(chroot_dir: &Path) -> PathBuf {
    chroot_dir.join("jail")
}

fn jailer_vm_root_dir(chroot_dir: &Path, jailer_id: &str) -> PathBuf {
    let firecracker_bin_name = Path::new(&firecracker_bin_path())
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "firecracker".to_string());

    jailer_chroot_base_dir(chroot_dir)
        .join(firecracker_bin_name)
        .join(jailer_id)
        .join("root")
}

fn vsock_uds_path(chroot_dir: &Path, workload_id: &str) -> PathBuf {
    let jailer_id = jailer_short_id(workload_id);
    jailer_vm_root_dir(chroot_dir, &jailer_id).join(format!("{}.vsock", API_SOCKET_NAME))
}

async fn connect_guest_vsock(
    chroot_dir: &Path,
    workload_id: &str,
    port: u32,
) -> Result<UnixStream> {
    let uds_path = vsock_uds_path(chroot_dir, workload_id);

    let mut stream = UnixStream::connect(&uds_path)
        .await
        .with_context(|| format!("Failed to connect to vsock uds {}", uds_path.display()))?;

    stream
        .write_all(format!("CONNECT {}\n", port).as_bytes())
        .await
        .context("Failed to write vsock CONNECT handshake")?;

    let mut response = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        stream
            .read_exact(&mut byte)
            .await
            .context("Failed to read vsock CONNECT response")?;
        if byte[0] == b'\n' {
            break;
        }
        response.push(byte[0]);
    }

    let response = String::from_utf8_lossy(&response);
    if !response.starts_with("OK ") {
        anyhow::bail!("vsock CONNECT to port {} refused: {}", port, response);
    }

    Ok(stream)
}

struct VmHandle {
    workload_id: String,
    jailer_pid: u32,
    chroot_dir: PathBuf,
    tap_device: String,
    vsock_cid: u32,
    resource_group_id: Option<String>,
    guest_ip: Option<String>,
    cgroup_path: PathBuf,
    metrics_path: PathBuf,
    previous_cpu_sample: Mutex<Option<(std::time::Instant, u64)>>,
}

struct GuestNetwork {
    ip: String,
    prefix: String,
    gateway: Option<String>,
    dns: Option<String>,
}

pub struct FirecrackerRuntime {
    wg_private_key_b64: String,
    dns_supervisor: RgDnsProcessSupervisor,
    rg_dns_registry: Arc<crate::rg_dns::RgDnsRegistry>,
    handles: Mutex<HashMap<String, VmHandle>>,
    next_cid: Mutex<u32>,
    reconciled: AtomicBool,
}

impl FirecrackerRuntime {
    pub fn new(
        wg_private_key_b64: String,
        rg_dns_registry: Arc<crate::rg_dns::RgDnsRegistry>,
    ) -> Self {
        Self {
            wg_private_key_b64,
            dns_supervisor: RgDnsProcessSupervisor::new(),
            rg_dns_registry,
            handles: Mutex::new(HashMap::new()),
            next_cid: Mutex::new(next_free_cid_on_host()),
            reconciled: AtomicBool::new(false),
        }
    }

    pub async fn reconcile_once(&self) -> Vec<(String, String)> {
        if self.reconciled.swap(true, Ordering::SeqCst) {
            return Vec::new();
        }

        match crate::runtime::Runtime::list_managed_workloads(self).await {
            Ok(recovered) => {
                info!(
                    count = recovered.len(),
                    "Reconciled running microvms from disk state"
                );
                recovered
            }
            Err(e) => {
                warn!(error = %e, "Failed to reconcile firecracker workloads from disk state");
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
            self.dns_supervisor
                .ensure_running(resource_group_id, cidr)
                .await
                .context("Failed to ensure resource group dns process")?;

            let listen_port = rg_wireguard_port(resource_group_id);
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

    async fn allocate_cid(&self) -> u32 {
        let mut next = self.next_cid.lock().await;
        let handles = self.handles.lock().await;

        let mut cid = *next;
        while handles.values().any(|h| h.vsock_cid == cid) {
            cid += 1;
        }

        *next = cid + 1;
        cid
    }

    pub async fn check_dns_liveness(&self) {
        self.dns_supervisor.check_liveness().await;
    }

    pub async fn teardown_rg_network(&self, resource_group_id: &str) -> Result<()> {
        self.dns_supervisor.stop(resource_group_id).await;

        crate::rg_network::teardown_bridge(resource_group_id)
            .await
            .context("Failed to tear down resource group bridge")?;

        crate::nftables::remove_bridge_rules(&crate::spec::rg_bridge_iface_name(resource_group_id))
            .await
            .context("Failed to remove nftables rules for resource group")?;

        let wg_iface = crate::wireguard::rg_interface_name(resource_group_id);
        crate::wireguard::remove_interface(&wg_iface)
            .await
            .context("Failed to remove resource group WireGuard interface")?;

        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::runtime::Runtime for FirecrackerRuntime {
    async fn pull_image(&self, _image: &str) -> Result<()> {
        Ok(())
    }

    async fn start_workload(&self, spec: &WorkloadSpec) -> Result<String> {
        debug!(workload_id = %spec.workload_id, image = %spec.image, stage = "start_workload", "Starting workload deploy");

        let rootfs_builder = RootfsBuilder::new();
        let rootfs_path = rootfs_builder.ensure_rootfs(&spec.image).await?;
        debug!(workload_id = %spec.workload_id, path = ?rootfs_path, stage = "rootfs_ready", "Rootfs image ready for boot");

        let jailer_id = jailer_short_id(&spec.workload_id);
        let chroot_dir = PathBuf::from(JAILER_BASE_DIR).join(&jailer_id);
        tokio::fs::create_dir_all(&chroot_dir)
            .await
            .context("Failed to create jailer chroot directory")?;

        let vsock_cid = self.allocate_cid().await;
        let tap_device = format!("fctap{}", vsock_cid);
        let vm_root_dir = jailer_vm_root_dir(&chroot_dir, &jailer_id);
        let api_socket_host_path = vm_root_dir
            .join(API_SOCKET_NAME)
            .to_string_lossy()
            .to_string();
        debug!(
            workload_id = %spec.workload_id,
            jailer_id = %jailer_id,
            vsock_cid = vsock_cid,
            tap_device = %tap_device,
            stage = "jailer_prepared",
            "Jailer chroot and vsock cid allocated"
        );

        let bridge_iface = match &spec.resource_group_id {
            Some(resource_group_id) => Some(
                self.ensure_rg_network(resource_group_id, spec.resource_group_cidr.as_deref())
                    .await?,
            ),
            None => None,
        };
        debug!(
            workload_id = %spec.workload_id,
            bridge_iface = ?bridge_iface,
            stage = "rg_network_ready",
            "Resource group network resolved"
        );

        let guest_network = match (&spec.resource_group_id, &spec.resource_group_cidr) {
            (Some(resource_group_id), Some(cidr)) => {
                let ip = crate::rg_ipam::allocate(resource_group_id, cidr, &spec.workload_id)
                    .await
                    .context("Failed to allocate resource group ip address")?;
                let prefix = cidr.split('/').nth(1).unwrap_or("24").to_string();
                let gateway = crate::spec::second_host_ip(cidr);
                let dns = crate::spec::second_host_ip(cidr);
                Some(GuestNetwork {
                    ip,
                    prefix,
                    gateway,
                    dns,
                })
            }
            _ => None,
        };
        debug!(
            workload_id = %spec.workload_id,
            guest_ip = ?guest_network.as_ref().map(|n| n.ip.clone()),
            stage = "guest_network_ready",
            "Guest network configuration resolved"
        );

        let jailer_uid = process_uid();

        let jailer_pid = spawn_jailer(&jailer_id, &chroot_dir, API_SOCKET_NAME, jailer_uid).await?;
        debug!(
            workload_id = %spec.workload_id,
            jailer_pid = jailer_pid,
            jailer_uid = jailer_uid,
            stage = "jailer_spawned",
            "Jailer process spawned"
        );

        let boot_result = boot_vm(
            &vm_root_dir,
            &rootfs_path,
            &tap_device,
            vsock_cid,
            &api_socket_host_path,
            bridge_iface.as_deref(),
            spec,
            guest_network.as_ref(),
            jailer_uid,
        )
        .await;

        let metrics_path = if let Err(e) = boot_result {
            debug!(workload_id = %spec.workload_id, error = %e, stage = "boot_vm_failed", "Firecracker boot failed, aborting jailer");
            abort_failed_jailer(&jailer_id, &tap_device).await;
            return Err(e);
        } else {
            debug!(workload_id = %spec.workload_id, stage = "boot_vm_ok", "Firecracker boot succeeded");
            vm_root_dir.join(METRICS_FILE_NAME)
        };

        let guest_ip = guest_network.as_ref().map(|net| net.ip.clone());

        let sidecar = VmSidecarMetadata {
            workload_id: spec.workload_id.clone(),
            vsock_cid,
            tap_device: tap_device.clone(),
            jailer_uid,
            resource_group_id: spec.resource_group_id.clone(),
            guest_ip: guest_ip.clone(),
        };
        write_sidecar_metadata(&chroot_dir, &sidecar)
            .await
            .context("Failed to write vm sidecar metadata")?;

        if let Some(guest_ip) = &guest_ip {
            debug!(workload_id = %spec.workload_id, guest_ip = %guest_ip, stage = "port_dnat", "Applying port forwarding rules");
            if let Err(e) = apply_port_dnat(&spec.workload_id, guest_ip, spec).await {
                warn!(workload_id = %spec.workload_id, error = %e, "Failed to apply port forwarding");
            }
        }

        self.handles.lock().await.insert(
            spec.workload_id.clone(),
            VmHandle {
                workload_id: spec.workload_id.clone(),
                jailer_pid,
                cgroup_path: cgroup_path(&spec.workload_id),
                metrics_path,
                chroot_dir,
                tap_device,
                vsock_cid,
                resource_group_id: spec.resource_group_id.clone(),
                guest_ip,
                previous_cpu_sample: Mutex::new(None),
            },
        );

        info!(workload_id = %spec.workload_id, vsock_cid = vsock_cid, "Firecracker microVM started");

        Ok(spec.workload_id.clone())
    }

    async fn inspect_status(&self, workload_handle: &str) -> Result<String> {
        let handles = self.handles.lock().await;
        let Some(handle) = handles.get(workload_handle) else {
            return Ok("failed".to_string());
        };

        let alive = tokio::fs::metadata(format!("/proc/{}", handle.jailer_pid))
            .await
            .is_ok();

        Ok(if alive {
            "running".to_string()
        } else {
            "failed".to_string()
        })
    }

    fn logs(&self, workload_handle: &str) -> LogStream {
        let jailer_id = jailer_short_id(workload_handle);
        let chroot_dir = PathBuf::from(JAILER_BASE_DIR).join(&jailer_id);

        Box::pin(vsock_log_stream(chroot_dir, workload_handle.to_string()))
    }

    async fn exec(&self, workload_handle: &str) -> Result<ExecSession> {
        let chroot_dir = {
            let handles = self.handles.lock().await;
            handles
                .get(workload_handle)
                .context("workload not running here")?
                .chroot_dir
                .clone()
        };

        let stream = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            connect_guest_vsock(&chroot_dir, workload_handle, 10002),
        )
        .await
        .context("Timed out connecting to guest exec vsock port")??;

        let (read_half, write_half) = stream.into_split();

        Ok(ExecSession {
            input: Box::pin(write_half),
            output: Box::pin(read_half),
        })
    }

    async fn stop_workload(&self, workload_handle: &str) -> Result<()> {
        let mut handles = self.handles.lock().await;
        let Some(handle) = handles.remove(workload_handle) else {
            return Ok(());
        };

        let _ = Command::new("systemctl")
            .args([
                "stop",
                &jailer_unit_name(&jailer_short_id(&handle.workload_id)),
            ])
            .status()
            .await;

        let _ = Command::new("ip")
            .args(["link", "delete", &handle.tap_device])
            .status()
            .await;

        if let Err(e) = remove_chroot_dir_privileged(&handle.chroot_dir).await {
            warn!(workload_id = %handle.workload_id, error = %e, "Failed to clean up jailer chroot directory");
        }

        if let Some(resource_group_id) = &handle.resource_group_id {
            crate::rg_ipam::release(resource_group_id, &handle.workload_id).await;
        }

        if let Err(e) = crate::nftables::remove_node_port_rules(&handle.workload_id).await {
            warn!(workload_id = %handle.workload_id, error = %e, "Failed to remove node port forwarding rules");
        }

        info!(workload_id = %handle.workload_id, "Firecracker microVM stopped");

        Ok(())
    }

    async fn stats(&self, workload_handle: &str) -> Result<crate::runtime::ContainerStats> {
        let (cpu_usage_percent, memory_usage_bytes, metrics_path) = {
            let handles = self.handles.lock().await;
            let Some(handle) = handles.get(workload_handle) else {
                return Ok(crate::runtime::ContainerStats::default());
            };
            let cpu_usage_percent = read_cpu_usage_percent(handle).await;
            let memory_usage_bytes = read_memory_usage_bytes(&handle.cgroup_path).await;
            (
                cpu_usage_percent,
                memory_usage_bytes,
                handle.metrics_path.clone(),
            )
        };

        let (network_rx_bytes, network_tx_bytes) = read_network_bytes(&metrics_path).await.unzip();

        Ok(crate::runtime::ContainerStats {
            cpu_usage_percent,
            memory_usage_bytes,
            network_rx_bytes,
            network_tx_bytes,
        })
    }

    async fn service_network_ip(
        &self,
        workload_handle: &str,
        _network_name: &str,
    ) -> Result<Option<String>> {
        let handles = self.handles.lock().await;
        Ok(handles
            .get(workload_handle)
            .and_then(|h| h.guest_ip.clone()))
    }

    async fn list_managed_workloads(&self) -> Result<Vec<(String, String)>> {
        let mut entries = match tokio::fs::read_dir(JAILER_BASE_DIR).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e).context("Failed to read jailer base directory"),
        };

        let mut recovered = Vec::new();

        while let Some(entry) = entries
            .next_entry()
            .await
            .context("Failed to read jailer base directory entry")?
        {
            let chroot_dir = entry.path();
            let Some(jailer_id) = entry.file_name().to_str().map(str::to_string) else {
                continue;
            };

            let Some(sidecar) = read_sidecar_metadata(&chroot_dir).await else {
                let _ = remove_chroot_dir_privileged(&chroot_dir).await;
                continue;
            };
            let workload_id = sidecar.workload_id.clone();

            let Some(jailer_pid) = find_live_jailer_pid(&jailer_id).await else {
                let _ = remove_chroot_dir_privileged(&chroot_dir).await;
                continue;
            };

            let metrics_path = jailer_vm_root_dir(&chroot_dir, &jailer_id).join("metrics.fifo");
            let handle = VmHandle {
                workload_id: workload_id.clone(),
                jailer_pid,
                cgroup_path: cgroup_path(&workload_id),
                metrics_path,
                chroot_dir,
                tap_device: sidecar.tap_device,
                vsock_cid: sidecar.vsock_cid,
                resource_group_id: sidecar.resource_group_id,
                guest_ip: sidecar.guest_ip,
                previous_cpu_sample: Mutex::new(None),
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

async fn read_cpu_usage_percent(handle: &VmHandle) -> Option<f64> {
    let usage_usec = read_cgroup_u64(&handle.cgroup_path, "cpu.stat", "usage_usec").await?;
    let now = std::time::Instant::now();

    let mut previous = handle.previous_cpu_sample.lock().await;
    let percent = match *previous {
        Some((previous_time, previous_usage)) => {
            let elapsed_usec = now.duration_since(previous_time).as_micros() as u64;
            if elapsed_usec == 0 || usage_usec < previous_usage {
                None
            } else {
                let delta_usec = usage_usec - previous_usage;
                Some((delta_usec as f64 / elapsed_usec as f64) * 100.0)
            }
        }
        None => None,
    };

    *previous = Some((now, usage_usec));
    percent
}

async fn read_memory_usage_bytes(cgroup_path: &Path) -> Option<i64> {
    let path = cgroup_path.join("memory.current");
    match tokio::fs::read_to_string(&path).await {
        Ok(content) => content.trim().parse::<i64>().ok(),
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to read workload memory cgroup");
            None
        }
    }
}

async fn read_cgroup_u64(cgroup_path: &Path, file: &str, key: &str) -> Option<u64> {
    let path = cgroup_path.join(file);
    let content = match tokio::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(e) => {
            warn!(path = %path.display(), error = %e, "failed to read workload cgroup file");
            return None;
        }
    };

    content.lines().find_map(|line| {
        let (line_key, value) = line.split_once(' ')?;
        if line_key == key {
            value.trim().parse::<u64>().ok()
        } else {
            None
        }
    })
}

async fn read_network_bytes(metrics_path: &Path) -> Option<(i64, i64)> {
    let content = tokio::time::timeout(
        std::time::Duration::from_millis(500),
        read_metrics_fifo_nonblocking(metrics_path),
    )
    .await
    .ok()??;

    let last_line = content.lines().last()?;
    let sample: serde_json::Value = serde_json::from_str(last_line).ok()?;

    let rx = sample.get("net")?.get("rx_bytes_count")?.as_i64()?;
    let tx = sample.get("net")?.get("tx_bytes_count")?.as_i64()?;

    Some((rx, tx))
}

async fn read_metrics_fifo_nonblocking(metrics_path: &Path) -> Option<String> {
    use std::os::unix::fs::OpenOptionsExt;
    use tokio::io::AsyncReadExt;

    let path = metrics_path.to_owned();
    let std_file = tokio::task::spawn_blocking(move || {
        std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(&path)
    })
    .await
    .ok()?
    .ok()?;

    let mut file = tokio::fs::File::from_std(std_file);
    let mut content = String::new();
    file.read_to_string(&mut content).await.ok()?;
    Some(content)
}

fn vsock_log_stream(
    chroot_dir: PathBuf,
    workload_id: String,
) -> impl futures_util::Stream<Item = Result<axum::body::Bytes, std::io::Error>> {
    async_stream::stream! {
        info!(workload_id = %workload_id, "connecting to guest log vsock");
        let connect_result = connect_guest_vsock(&chroot_dir, &workload_id, 10001).await;

        let mut stream = match connect_result {
            Ok(s) => s,
            Err(e) => {
                warn!(workload_id = %workload_id, error = ?e, "failed to connect to guest log vsock");
                yield Err(std::io::Error::other(e));
                return;
            }
        };
        info!(workload_id = %workload_id, "guest log vsock connected");

        let mut buf = [0u8; 4096];
        let mut total_bytes: u64 = 0;
        loop {
            match stream.read(&mut buf).await {
                Ok(0) => {
                    info!(workload_id = %workload_id, total_bytes, "guest log vsock closed by peer");
                    break;
                }
                Ok(n) => {
                    total_bytes += n as u64;
                    yield Ok(axum::body::Bytes::copy_from_slice(&buf[..n]));
                }
                Err(e) => {
                    warn!(workload_id = %workload_id, total_bytes, error = ?e, "guest log vsock read failed");
                    yield Err(e);
                    break;
                }
            }
        }
    }
}

fn jailer_unit_name(jailer_id: &str) -> String {
    format!("csfx-jailer-{}", jailer_id)
}

#[allow(clippy::too_many_arguments)]
async fn boot_vm(
    vm_root_dir: &Path,
    rootfs_path: &Path,
    tap_device: &str,
    vsock_cid: u32,
    api_socket_host_path: &str,
    bridge_iface: Option<&str>,
    spec: &WorkloadSpec,
    guest_network: Option<&GuestNetwork>,
    jailer_uid: u32,
) -> Result<()> {
    let metrics_path = vm_root_dir.join(METRICS_FILE_NAME);
    create_metrics_fifo(&metrics_path).context("Failed to create metrics fifo")?;

    debug!(path = ?rootfs_path, vm_root_dir = ?vm_root_dir, stage = "stage_kernel", "Staging kernel image into jailer chroot");
    stage_kernel_into_jail(vm_root_dir)
        .await
        .context("Failed to stage kernel image into jailer chroot")?;

    debug!(path = ?rootfs_path, stage = "stage_rootfs", "Staging rootfs image into jailer chroot");
    stage_rootfs_into_jail(rootfs_path, vm_root_dir)
        .await
        .context("Failed to stage rootfs image into jailer chroot")?;

    let boot_config = VmBootConfig {
        api_socket: api_socket_host_path.to_string(),
        metrics_socket_name: METRICS_FILE_NAME.to_string(),
        tap_device: tap_device.to_string(),
        vsock_cid,
    };
    debug!(
        api_socket = %boot_config.api_socket,
        tap_device = %boot_config.tap_device,
        vsock_cid = boot_config.vsock_cid,
        stage = "boot_config_ready",
        "Boot config assembled, calling Firecracker API"
    );

    configure_and_boot_vm(
        &boot_config,
        vm_root_dir,
        bridge_iface,
        spec,
        guest_network,
        jailer_uid,
    )
    .await
}

async fn abort_failed_jailer(jailer_id: &str, tap_device: &str) {
    let _ = Command::new("systemctl")
        .args(["stop", &jailer_unit_name(jailer_id)])
        .status()
        .await;

    let _ = Command::new("ip")
        .args(["link", "delete", tap_device])
        .status()
        .await;
}

async fn spawn_jailer(
    jailer_id: &str,
    chroot_dir: &Path,
    api_socket: &str,
    jailer_uid: u32,
) -> Result<u32> {
    let uid_arg = jailer_uid.to_string();
    let gid_arg = process_gid().to_string();
    let firecracker_bin = firecracker_bin_path();
    let chroot_base_dir = jailer_chroot_base_dir(chroot_dir);
    if chroot_base_dir.exists() {
        remove_chroot_dir_privileged(&chroot_base_dir).await?;
    }
    tokio::fs::create_dir_all(&chroot_base_dir)
        .await
        .context("Failed to create jailer chroot base directory")?;

    let unit_name = jailer_unit_name(jailer_id);
    let status = Command::new("systemd-run")
        .args([
            "--unit",
            &unit_name,
            "--collect",
            "--uid=0",
            "--gid=0",
            "--property=StandardOutput=journal",
            "--property=StandardError=journal",
            "jailer",
            "--id",
            jailer_id,
            "--exec-file",
            &firecracker_bin,
            "--uid",
            &uid_arg,
            "--gid",
            &gid_arg,
            "--chroot-base-dir",
            chroot_base_dir.to_string_lossy().as_ref(),
            "--cgroup-version",
            "2",
            "--parent-cgroup",
            CGROUP_PARENT,
            "--",
            "--api-sock",
            api_socket,
            "--no-seccomp",
        ])
        .status()
        .await
        .context("Failed to spawn jailer via systemd-run")?;

    if !status.success() {
        anyhow::bail!("systemd-run failed to start jailer unit {}", unit_name);
    }

    for _ in 0..250 {
        if let Some(pid) = find_live_jailer_pid(jailer_id).await {
            return Ok(pid);
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }

    anyhow::bail!("jailer process for id {} did not appear", jailer_id)
}

async fn remove_chroot_dir_privileged(path: &Path) -> Result<()> {
    let slug: String = path
        .to_string_lossy()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let unit = format!("csfx-jailer-cleanup-{slug}");

    let status = Command::new("systemd-run")
        .args([
            "--pipe",
            "--wait",
            "--collect",
            "--uid=0",
            "--gid=0",
            &format!("--unit={unit}"),
            "rm",
            "-rf",
            "--",
        ])
        .arg(path)
        .status()
        .await
        .context("Failed to spawn privileged chroot cleanup")?;

    if !status.success() {
        anyhow::bail!("privileged cleanup of chroot directory {:?} failed", path);
    }

    Ok(())
}

async fn stage_kernel_into_jail(vm_root_dir: &Path) -> Result<()> {
    let source = tokio::fs::canonicalize(GUEST_KERNEL_PATH)
        .await
        .context("Failed to resolve kernel image path")?;
    reflink_copy(&source, &vm_root_dir.join(JAIL_KERNEL_NAME))
        .await
        .context("Failed to copy kernel image into jailer chroot")
}

async fn stage_rootfs_into_jail(rootfs_path: &Path, vm_root_dir: &Path) -> Result<()> {
    reflink_copy(rootfs_path, &vm_root_dir.join(JAIL_ROOTFS_NAME))
        .await
        .context("Failed to copy rootfs image into jailer chroot")
}

async fn reflink_copy(source: &Path, destination: &Path) -> Result<()> {
    let status = Command::new("cp")
        .args(["--reflink=auto", "--sparse=always"])
        .arg(source)
        .arg(destination)
        .status()
        .await
        .context("Failed to execute cp")?;
    if !status.success() {
        anyhow::bail!("cp failed from {:?} to {:?}", source, destination);
    }
    Ok(())
}

fn create_metrics_fifo(path: &Path) -> Result<()> {
    let path_cstr = std::ffi::CString::new(path.as_os_str().as_encoded_bytes())
        .context("Invalid metrics fifo path")?;

    let result = unsafe { libc::mkfifo(path_cstr.as_ptr(), 0o600) };
    if result != 0 {
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::AlreadyExists {
            return Err(err).context("mkfifo failed for metrics path");
        }
    }

    Ok(())
}

fn cgroup_path(workload_id: &str) -> PathBuf {
    let unit_name = jailer_unit_name(&jailer_short_id(workload_id));
    Path::new(CGROUP_ROOT)
        .join("system.slice")
        .join(format!("{}.service", unit_name))
}

async fn apply_port_dnat(workload_id: &str, guest_ip: &str, spec: &WorkloadSpec) -> Result<()> {
    let Some(ports) = &spec.ports else {
        return Ok(());
    };

    let rg_gateway_ip = spec
        .resource_group_cidr
        .as_deref()
        .and_then(crate::spec::second_host_ip);

    for port in ports {
        let protocol = port.protocol.as_deref().unwrap_or("tcp");

        if let Some(node_port) = port.node_port {
            crate::nftables::add_node_port_dnat(
                workload_id,
                protocol,
                node_port,
                guest_ip,
                port.container_port,
            )
            .await
            .context("Failed to add node port dnat rule")?;
        }

        if let Some(rg_port) = port.rg_port {
            let rg_gateway_ip = rg_gateway_ip
                .as_deref()
                .context("rg_port requires a resource group with a cidr")?;
            crate::nftables::add_rg_port_dnat(
                workload_id,
                protocol,
                rg_gateway_ip,
                rg_port,
                guest_ip,
                port.container_port,
            )
            .await
            .context("Failed to add rg port dnat rule")?;
        }
    }

    Ok(())
}

#[derive(serde::Serialize, serde::Deserialize)]
struct VmSidecarMetadata {
    workload_id: String,
    vsock_cid: u32,
    tap_device: String,
    jailer_uid: u32,
    resource_group_id: Option<String>,
    guest_ip: Option<String>,
}

fn sidecar_metadata_path(chroot_dir: &Path) -> PathBuf {
    chroot_dir.join("csfx-meta.json")
}

async fn write_sidecar_metadata(chroot_dir: &Path, metadata: &VmSidecarMetadata) -> Result<()> {
    let payload = serde_json::to_vec(metadata).context("Failed to serialize sidecar metadata")?;
    tokio::fs::write(sidecar_metadata_path(chroot_dir), payload)
        .await
        .context("Failed to write sidecar metadata file")?;
    Ok(())
}

async fn read_sidecar_metadata(chroot_dir: &Path) -> Option<VmSidecarMetadata> {
    let content = tokio::fs::read(sidecar_metadata_path(chroot_dir))
        .await
        .ok()?;
    serde_json::from_slice(&content).ok()
}

async fn find_live_jailer_pid(jailer_id: &str) -> Option<u32> {
    let mut proc_entries = tokio::fs::read_dir("/proc").await.ok()?;

    while let Ok(Some(entry)) = proc_entries.next_entry().await {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|n| n.parse::<u32>().ok())
        else {
            continue;
        };

        let cmdline_path = entry.path().join("cmdline");
        let Ok(cmdline_raw) = tokio::fs::read(&cmdline_path).await else {
            continue;
        };

        let args: Vec<&str> = cmdline_raw
            .split(|b| *b == 0)
            .filter_map(|part| std::str::from_utf8(part).ok())
            .filter(|part| !part.is_empty())
            .collect();

        let is_jailer_process = args
            .first()
            .map(|a| a.ends_with("jailer") || a.ends_with("firecracker"))
            .unwrap_or(false);
        if !is_jailer_process {
            continue;
        }

        let matches_id = args
            .iter()
            .zip(args.iter().skip(1))
            .any(|(flag, value)| *flag == "--id" && *value == jailer_id);

        if matches_id {
            return Some(pid);
        }
    }

    None
}

struct VmBootConfig {
    api_socket: String,
    tap_device: String,
    metrics_socket_name: String,
    vsock_cid: u32,
}

async fn configure_and_boot_vm(
    boot_config: &VmBootConfig,
    vm_root_dir: &Path,
    bridge_iface: Option<&str>,
    spec: &WorkloadSpec,
    guest_network: Option<&GuestNetwork>,
    jailer_uid: u32,
) -> Result<()> {
    debug!(
        tap_device = %boot_config.tap_device,
        bridge_iface = ?bridge_iface,
        stage = "create_tap_device",
        "Creating tap device"
    );
    create_tap_device(&boot_config.tap_device, bridge_iface, jailer_uid).await?;

    let client = FirecrackerApiClient::new(boot_config.api_socket.clone());

    let vcpu_count = ((spec.cpu_millicores.max(100)) as f64 / 1000.0).ceil() as i64;
    let mem_size_mib = (spec.memory_bytes / 1024 / 1024).max(128);

    debug!(
        vcpu_count = vcpu_count,
        mem_size_mib = mem_size_mib,
        stage = "machine_config",
        "Configuring machine"
    );
    client
        .put(
            "/machine-config",
            &json!({
                "vcpu_count": vcpu_count,
                "mem_size_mib": mem_size_mib,
            }),
        )
        .await?;

    debug!(stage = "metrics_config", "Configuring metrics socket");
    client
        .put(
            "/metrics",
            &json!({
                "metrics_path": boot_config.metrics_socket_name,
            }),
        )
        .await
        .context("Failed to configure metrics")?;

    debug!(stage = "boot_source_config", "Configuring boot source");
    client
        .put(
            "/boot-source",
            &json!({
                "kernel_image_path": JAIL_KERNEL_NAME,
                "boot_args": "console=ttyS0 reboot=k panic=1 pci=off init=/sbin/csfx-guest-init cgroup_no_v1=all systemd.unified_cgroup_hierarchy=1",
            }),
        )
        .await?;

    debug!(stage = "rootfs_drive_config", "Configuring rootfs drive");
    client
        .put(
            "/drives/rootfs",
            &json!({
                "drive_id": "rootfs",
                "path_on_host": JAIL_ROOTFS_NAME,
                "is_root_device": true,
                "is_read_only": false,
            }),
        )
        .await?;

    debug!(
        volume_count = spec.volume_mounts.as_deref().unwrap_or_default().len(),
        stage = "volume_drives_config",
        "Attaching volume drives"
    );
    let mounted_volumes = attach_volume_drives(
        &client,
        vm_root_dir,
        spec.volume_mounts.as_deref().unwrap_or_default(),
    )
    .await?;

    debug!(
        stage = "network_interface_config",
        "Configuring network interface"
    );
    client
        .put(
            "/network-interfaces/eth0",
            &json!({
                "iface_id": "eth0",
                "host_dev_name": boot_config.tap_device,
            }),
        )
        .await?;

    debug!(
        vsock_cid = boot_config.vsock_cid,
        stage = "vsock_config",
        "Configuring vsock device"
    );
    client
        .put(
            "/vsock",
            &json!({
                "guest_cid": boot_config.vsock_cid,
                "uds_path": format!("{}.vsock", API_SOCKET_NAME),
            }),
        )
        .await?;

    debug!(stage = "mmds_config", "Configuring mmds");
    configure_mmds(
        &client,
        &mounted_volumes,
        guest_network,
        spec.env_vars.as_ref(),
    )
    .await?;

    debug!(stage = "instance_start", "Sending InstanceStart action");
    client
        .put("/actions", &json!({ "action_type": "InstanceStart" }))
        .await?;

    debug!(
        stage = "instance_started",
        "InstanceStart accepted by Firecracker"
    );

    Ok(())
}

struct MountedVolume {
    guest_device: String,
    mount_path: String,
}

async fn attach_volume_drives(
    client: &FirecrackerApiClient,
    vm_root_dir: &Path,
    volume_mounts: &[crate::spec::VolumeMount],
) -> Result<Vec<MountedVolume>> {
    let mut mounted = Vec::with_capacity(volume_mounts.len());

    for (index, volume_mount) in volume_mounts.iter().enumerate() {
        let guest_device = guest_device_letter(index);
        let jail_device_name = format!("vd-{}", volume_mount.volume_id);

        stage_block_device_into_jail(&volume_mount.device_path, vm_root_dir, &jail_device_name)
            .await
            .context("Failed to stage volume block device into jailer chroot")?;

        client
            .put(
                &format!("/drives/{}", volume_mount.volume_id),
                &json!({
                    "drive_id": volume_mount.volume_id,
                    "path_on_host": jail_device_name,
                    "is_root_device": false,
                    "is_read_only": false,
                }),
            )
            .await
            .context("Failed to attach volume drive")?;

        mounted.push(MountedVolume {
            guest_device,
            mount_path: volume_mount.mount_path.clone(),
        });
    }

    Ok(mounted)
}

async fn stage_block_device_into_jail(
    device_path: &str,
    vm_root_dir: &Path,
    jail_device_name: &str,
) -> Result<()> {
    let metadata = tokio::fs::metadata(device_path)
        .await
        .context("Failed to stat volume block device")?;
    let device_id = std::os::unix::fs::MetadataExt::rdev(&metadata) as libc::dev_t;
    let major = libc::major(device_id);
    let minor = libc::minor(device_id);
    let jailer_uid = process_uid();
    let jailer_gid = process_gid();
    let target_path = vm_root_dir.join(jail_device_name);

    let status = Command::new("systemd-run")
        .args([
            "--pipe",
            "--wait",
            "--collect",
            "--uid=0",
            "--gid=0",
            "sh",
            "-c",
        ])
        .arg(format!(
            "rm -f {target} && mknod {target} b {major} {minor} && chown {uid}:{gid} {target}",
            target = target_path.to_string_lossy(),
            major = major,
            minor = minor,
            uid = jailer_uid,
            gid = jailer_gid,
        ))
        .status()
        .await
        .context("Failed to spawn privileged mknod for volume device")?;

    if !status.success() {
        anyhow::bail!(
            "privileged mknod failed for volume device {:?}",
            target_path
        );
    }

    Ok(())
}

fn guest_device_letter(index: usize) -> String {
    let letter = (b'b' + index as u8) as char;
    format!("/dev/vd{}", letter)
}

async fn configure_mmds(
    client: &FirecrackerApiClient,
    volumes: &[MountedVolume],
    guest_network: Option<&GuestNetwork>,
    env_vars: Option<&HashMap<String, String>>,
) -> Result<()> {
    client
        .put(
            "/mmds/config",
            &json!({
                "version": "V1",
                "network_interfaces": ["eth0"],
            }),
        )
        .await
        .context("Failed to configure mmds")?;

    let volumes_payload: Vec<serde_json::Value> = volumes
        .iter()
        .map(|v| {
            json!({
                "device": v.guest_device,
                "mount_path": v.mount_path,
            })
        })
        .collect();

    let network_payload = guest_network.map(|net| {
        json!({
            "ip": net.ip,
            "prefix": net.prefix,
            "gateway": net.gateway,
            "dns": net.dns,
        })
    });

    client
        .put(
            "/mmds",
            &json!({
                "volumes": volumes_payload,
                "network": network_payload,
                "env": env_vars.cloned().unwrap_or_default(),
            }),
        )
        .await
        .context("Failed to write mmds data")?;

    Ok(())
}

async fn create_tap_device(
    tap_device: &str,
    bridge_iface: Option<&str>,
    owner_uid: u32,
) -> Result<()> {
    let uid_arg = owner_uid.to_string();
    let status = Command::new("ip")
        .args([
            "tuntap", "add", "dev", tap_device, "mode", "tap", "user", &uid_arg,
        ])
        .status()
        .await
        .context("Failed to execute ip tuntap add")?;

    if !status.success() {
        anyhow::bail!("failed to create tap device {}", tap_device);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sidecar_metadata_round_trips() {
        let dir = std::env::temp_dir().join(format!("csfx-sidecar-test-{}", uuid::Uuid::new_v4()));
        tokio::fs::create_dir_all(&dir).await.unwrap();

        let metadata = VmSidecarMetadata {
            workload_id: "workload-1".to_string(),
            vsock_cid: 1234,
            tap_device: "fctap1234".to_string(),
            jailer_uid: 60001,
            resource_group_id: Some("rg-1".to_string()),
            guest_ip: Some("10.0.0.3".to_string()),
        };

        write_sidecar_metadata(&dir, &metadata).await.unwrap();
        let read_back = read_sidecar_metadata(&dir).await.unwrap();

        assert_eq!(read_back.workload_id, metadata.workload_id);
        assert_eq!(read_back.vsock_cid, metadata.vsock_cid);
        assert_eq!(read_back.tap_device, metadata.tap_device);
        assert_eq!(read_back.jailer_uid, metadata.jailer_uid);
        assert_eq!(read_back.resource_group_id, metadata.resource_group_id);
        assert_eq!(read_back.guest_ip, metadata.guest_ip);

        tokio::fs::remove_dir_all(&dir).await.unwrap();
    }

    #[tokio::test]
    async fn missing_sidecar_metadata_returns_none() {
        let dir = std::env::temp_dir().join(format!("csfx-sidecar-test-{}", uuid::Uuid::new_v4()));
        assert!(read_sidecar_metadata(&dir).await.is_none());
    }
}
