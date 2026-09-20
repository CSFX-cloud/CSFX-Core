mod agent_stream;
mod client;
mod config;
mod disks;
mod firecracker;
mod nftables;
mod pki;
mod qemu;
mod rbd;
mod rg_dhcp;
mod rg_dns;
mod rg_dns_process;
mod rg_ipam;
mod rg_network;
mod runtime;
mod server;
mod spec;
mod ssh_keys;
mod system;
mod update_watch;
mod wg_identity;
mod wireguard;

use anyhow::{Context, Result};
use runtime::Runtime;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("failed to install ring crypto provider");

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    let gateway_url = std::env::var("CSFX_GATEWAY_URL")
        .context("CSFX_GATEWAY_URL environment variable is required")?;

    if let Some(username) = std::env::args()
        .nth(1)
        .filter(|a| a == "--authorized-keys")
        .and_then(|_| std::env::args().nth(2))
    {
        let agent_id = config::load_config()
            .context("Failed to load daemon config")?
            .agent_id;
        ssh_keys::run_authorized_keys_command(&gateway_url, agent_id).await;
        let _ = username;
        return Ok(());
    }

    info!(version = env!("CARGO_PKG_VERSION"), "csfx-agent starting");

    if !system::is_kvm_capable() {
        error!("no /dev/kvm found, this host cannot schedule any workloads");
    }

    let heartbeat_interval_secs: u64 = std::env::var("CSFX_HEARTBEAT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(30);

    let wg_endpoint = std::env::var("CSFX_WG_ENDPOINT")
        .ok()
        .or_else(detect_wg_endpoint);
    let wg_tunnel_ip = if config::is_registered() {
        config::load_config().ok().and_then(|cfg| cfg.wg_tunnel_ip)
    } else {
        std::env::var("CSFX_WG_TUNNEL_IP").ok()
    };
    let wg_identity =
        wg_identity::load_or_generate().context("Failed to initialize WireGuard identity")?;

    let api_client = client::ApiClient::new(
        gateway_url.clone(),
        wg_identity.public_key_b64.clone(),
        wg_endpoint,
        wg_tunnel_ip,
    )
    .context("Failed to initialize API client")?;

    let agent_pki = pki::AgentPki::load_or_generate().context("Failed to initialize PKI")?;

    let (agent_id, api_key) = if config::is_registered() {
        info!("Existing registration found, loading credentials");
        let cfg = config::load_config().context("Failed to load daemon config")?;
        let creds = config::load_credentials().context("Failed to load credentials")?;
        (cfg.agent_id, creds.api_key)
    } else {
        info!("No registration found, starting registration");
        perform_registration(
            &api_client,
            &gateway_url,
            heartbeat_interval_secs,
            &agent_pki,
        )
        .await?
    };

    let api_client = if pki::AgentPki::has_certificate() {
        match pki::AgentPki::load_cert_pem() {
            Ok(cert_pem) => {
                info!("mTLS: client certificate loaded");
                api_client.with_certificate(cert_pem)
            }
            Err(e) => {
                warn!(error = %e, "mTLS: failed to load certificate, continuing without");
                api_client
            }
        }
    } else {
        api_client
    };

    info!(agent_id = %agent_id, "Agent registered, starting heartbeat loop");

    let mgmt_tunnel_ip = config::load_config().ok().and_then(|cfg| cfg.wg_tunnel_ip);
    if let Some(ref tunnel_ip) = mgmt_tunnel_ip {
        if let Err(e) =
            wireguard::ensure_mgmt_interface(&wg_identity.private_key_b64, MGMT_WG_PORT, tunnel_ip)
                .await
        {
            warn!(error = %e, "Failed to bring up management WireGuard interface");
        }
    } else {
        warn!("No management tunnel IP available, VPN peering disabled for this agent");
    }

    if let Err(e) = nftables::ensure_table_and_chain().await {
        warn!(error = %e, "Failed to initialize nftables resource group isolation");
    }

    let rg_dns_registry = Arc::new(rg_dns::RgDnsRegistry::new());

    let firecracker_runtime = Arc::new(firecracker::runtime::FirecrackerRuntime::new(
        wg_identity.private_key_b64.clone(),
        Arc::clone(&rg_dns_registry),
    ));

    let qemu_runtime = Arc::new(qemu::runtime::QemuRuntime::new(
        wg_identity.private_key_b64.clone(),
        Arc::clone(&rg_dns_registry),
    ));

    let running_containers: Arc<Mutex<HashMap<String, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let vm_workload_ids: Arc<Mutex<std::collections::HashSet<String>>> =
        Arc::new(Mutex::new(std::collections::HashSet::new()));

    let workload_phases: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    let mounted_volumes: Arc<Mutex<HashMap<String, String>>> = Arc::new(Mutex::new(HashMap::new()));

    let restart_counts: Arc<Mutex<HashMap<String, u32>>> = Arc::new(Mutex::new(HashMap::new()));

    let service_dns_registry: Arc<Mutex<HashMap<String, (String, String)>>> =
        Arc::new(Mutex::new(HashMap::new()));

    if let Some(port) = std::env::var("CSFX_AGENT_PORT")
        .ok()
        .and_then(|v| v.parse::<u16>().ok())
    {
        let server_state = server::ServerState {
            firecracker: firecracker_runtime.clone(),
            qemu: qemu_runtime.clone(),
            running_containers: running_containers.clone(),
            agent_id,
        };
        tokio::spawn(async move {
            if let Err(e) = server::run(server_state, port).await {
                error!(error = %e, "agent inbound server stopped");
            }
        });
    } else {
        info!("CSFX_AGENT_PORT not set, agent inbound server disabled");
    }

    let assignment_signal = agent_stream::spawn(gateway_url.clone(), api_key.clone(), agent_id);

    run_heartbeat_loop(
        &api_client,
        agent_id,
        &api_key,
        heartbeat_interval_secs,
        firecracker_runtime,
        qemu_runtime,
        running_containers,
        vm_workload_ids,
        workload_phases,
        mounted_volumes,
        restart_counts,
        service_dns_registry,
        rg_dns_registry,
        assignment_signal,
    )
    .await;

    Ok(())
}

async fn perform_registration(
    client: &client::ApiClient,
    gateway_url: &str,
    heartbeat_interval_secs: u64,
    agent_pki: &pki::AgentPki,
) -> Result<(uuid::Uuid, String)> {
    let token = match std::env::var("CSFX_REGISTRATION_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
    {
        Some(t) => t,
        None => {
            info!("CSFX_REGISTRATION_TOKEN not set, fetching bootstrap token from gateway");
            client
                .fetch_bootstrap_token()
                .await
                .context("Failed to fetch bootstrap token from gateway")?
        }
    };

    let info = system::collect_info();

    info!(
        hostname = %info.hostname,
        os_type = %info.os_type,
        architecture = %info.architecture,
        "Registering with registry"
    );

    let resp = client
        .register(
            &token,
            &info.hostname,
            &info.hostname,
            &info.os_type,
            &info.os_version,
            &info.architecture,
            agent_pki.csr_pem(),
        )
        .await
        .context("Registration request failed")?;

    if let (Some(cert_pem), Some(ca_pem)) = (&resp.certificate_pem, &resp.ca_cert_pem) {
        pki::AgentPki::save_certificate(cert_pem, ca_pem).context("Failed to save certificate")?;
        info!("PKI: certificate received and stored");
    } else {
        warn!("Registry did not issue a certificate during registration");
    }

    if let Some(ref ip) = resp.wg_tunnel_ip {
        client.set_wg_tunnel_ip(ip.clone()).await;
        info!(wg_tunnel_ip = %ip, "Management tunnel IP assigned by registry");
    } else {
        warn!("Registry did not assign a management tunnel IP");
    }

    let cfg = config::DaemonConfig {
        gateway_url: gateway_url.to_string(),
        agent_id: resp.agent_id,
        heartbeat_interval_secs,
        wg_tunnel_ip: resp.wg_tunnel_ip.clone(),
    };

    config::save_config(&cfg).context("Failed to save daemon config")?;
    config::save_credentials(&resp.api_key).context("Failed to save credentials")?;

    info!(agent_id = %resp.agent_id, "Registration successful");

    Ok((resp.agent_id, resp.api_key))
}

#[allow(clippy::too_many_arguments)]
async fn reconcile_tick(
    client: &client::ApiClient,
    agent_id: uuid::Uuid,
    api_key: &str,
    firecracker: &Arc<firecracker::runtime::FirecrackerRuntime>,
    qemu: &Arc<qemu::runtime::QemuRuntime>,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
    vm_workload_ids: &Arc<Mutex<std::collections::HashSet<String>>>,
    workload_phases: &Arc<Mutex<HashMap<String, String>>>,
    mounted_volumes: &Arc<Mutex<HashMap<String, String>>>,
    restart_counts: &Arc<Mutex<HashMap<String, u32>>>,
    service_dns_registry: &Arc<Mutex<HashMap<String, (String, String)>>>,
    rg_dns_registry: &rg_dns::RgDnsRegistry,
    failure_count: &mut u32,
    current_flake_rev: &mut String,
) {
    process_volumes(client, agent_id, api_key, mounted_volumes).await;
    let resource_group_ids = process_workloads(
        client,
        api_key,
        firecracker,
        qemu,
        running_containers,
        vm_workload_ids,
        workload_phases,
        mounted_volumes,
        restart_counts,
        service_dns_registry,
        rg_dns_registry,
    )
    .await;
    sync_wireguard_peers(client, api_key, agent_id, &resource_group_ids).await;
    sync_vpn_peers(client, api_key, &resource_group_ids).await;
    firecracker.check_dns_liveness().await;
    qemu.check_dhcp_liveness().await;
    cleanup_stale_resource_groups(client, api_key, firecracker, qemu).await;

    let statuses = build_container_statuses(
        firecracker,
        qemu,
        running_containers,
        vm_workload_ids,
        workload_phases,
    )
    .await;
    push_workload_stats(client, api_key, firecracker, running_containers).await;
    let metrics = system::collect_metrics();

    match client
        .heartbeat(agent_id, api_key, Some(statuses), Some(metrics))
        .await
    {
        Ok(resp) => {
            if *failure_count > 0 {
                info!(agent_id = %agent_id, "Heartbeat recovered after {} failures", failure_count);
                *failure_count = 0;
            }

            info!(
                agent_id = %agent_id,
                desired_flake_rev = ?resp.desired_flake_rev,
                "heartbeat ok"
            );

            if let Some(count) = resp.post_update_heartbeats {
                update_watch::write_heartbeat_counter(count).await;
            }

            if let Some(rev) = resp.desired_flake_rev {
                if rev != *current_flake_rev {
                    info!(
                        agent_id = %agent_id,
                        current_flake_rev = %current_flake_rev,
                        desired_flake_rev = %rev,
                        "update signal received from gateway, scheduling update"
                    );
                }
                let rev_clone = rev.clone();
                let current = current_flake_rev.clone();
                tokio::spawn(async move {
                    update_watch::handle(agent_id, &rev_clone, &current).await;
                });
                *current_flake_rev = rev;
            }
        }
        Err(e) => {
            *failure_count += 1;
            warn!(
                agent_id = %agent_id,
                failures = *failure_count,
                error = %e,
                "Heartbeat failed"
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
async fn run_heartbeat_loop(
    client: &client::ApiClient,
    agent_id: uuid::Uuid,
    api_key: &str,
    interval_secs: u64,
    firecracker: Arc<firecracker::runtime::FirecrackerRuntime>,
    qemu: Arc<qemu::runtime::QemuRuntime>,
    running_containers: Arc<Mutex<HashMap<String, String>>>,
    vm_workload_ids: Arc<Mutex<std::collections::HashSet<String>>>,
    workload_phases: Arc<Mutex<HashMap<String, String>>>,
    mounted_volumes: Arc<Mutex<HashMap<String, String>>>,
    restart_counts: Arc<Mutex<HashMap<String, u32>>>,
    service_dns_registry: Arc<Mutex<HashMap<String, (String, String)>>>,
    rg_dns_registry: Arc<rg_dns::RgDnsRegistry>,
    mut assignment_signal: tokio::sync::mpsc::UnboundedReceiver<()>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    let mut failure_count: u32 = 0;
    let mut current_flake_rev = String::new();

    loop {
        tokio::select! {
            _ = interval.tick() => {
                reconcile_tick(client, agent_id, api_key, &firecracker, &qemu, &running_containers, &vm_workload_ids, &workload_phases, &mounted_volumes, &restart_counts, &service_dns_registry, &rg_dns_registry, &mut failure_count, &mut current_flake_rev).await;
            }
            signal = assignment_signal.recv() => {
                if signal.is_none() {
                    warn!(agent_id = %agent_id, "assignment signal channel closed");
                    continue;
                }
                info!(agent_id = %agent_id, "assignment push received, reconciling immediately");
                interval.reset();
                reconcile_tick(client, agent_id, api_key, &firecracker, &qemu, &running_containers, &vm_workload_ids, &workload_phases, &mounted_volumes, &restart_counts, &service_dns_registry, &rg_dns_registry, &mut failure_count, &mut current_flake_rev).await;
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    if failure_count > 0 {
        error!(
            failures = failure_count,
            "Agent shutting down with unresolved heartbeat failures"
        );
    }
}

async fn process_volumes(
    client: &client::ApiClient,
    agent_id: uuid::Uuid,
    api_key: &str,
    mounted_volumes: &Arc<Mutex<HashMap<String, String>>>,
) {
    let volumes = match client.fetch_assigned_volumes(agent_id, api_key).await {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "Failed to fetch assigned volumes");
            return;
        }
    };

    for volume in volumes {
        let already_mounted = mounted_volumes.lock().await.contains_key(&volume.id);
        if already_mounted {
            continue;
        }

        info!(volume_id = %volume.id, image = %volume.image_name, "Mapping volume");

        let device = match rbd::map_device(&volume.pool, &volume.image_name).await {
            Ok(d) => d,
            Err(e) => {
                warn!(volume_id = %volume.id, error = %e, "Failed to map RBD device");
                continue;
            }
        };

        mounted_volumes
            .lock()
            .await
            .insert(volume.id.clone(), device.clone());

        info!(volume_id = %volume.id, device = %device, "Volume mapped");
    }
}

#[allow(clippy::too_many_arguments)]
async fn reap_stale_containers(
    firecracker: &dyn runtime::Runtime,
    qemu: &dyn runtime::Runtime,
    vm_workload_ids: &Arc<Mutex<std::collections::HashSet<String>>>,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
    workload_phases: &Arc<Mutex<HashMap<String, String>>>,
    restart_counts: &Arc<Mutex<HashMap<String, u32>>>,
    service_dns_registry: &Arc<Mutex<HashMap<String, (String, String)>>>,
    rg_dns_registry: &rg_dns::RgDnsRegistry,
    desired: &[client::AssignedWorkload],
) {
    let desired_ids: std::collections::HashSet<&str> =
        desired.iter().map(|w| w.id.as_str()).collect();

    let stale: Vec<(String, String)> = running_containers
        .lock()
        .await
        .iter()
        .filter(|(workload_id, _)| !desired_ids.contains(workload_id.as_str()))
        .map(|(workload_id, container_id)| (workload_id.clone(), container_id.clone()))
        .collect();

    for (workload_id, container_id) in stale {
        info!(workload_id = %workload_id, container_id = %container_id, "Tearing down removed workload");

        let is_vm = vm_workload_ids.lock().await.contains(&workload_id);
        let runtime = if is_vm { qemu } else { firecracker };

        if let Err(e) = runtime.stop_workload(&container_id).await {
            warn!(workload_id = %workload_id, container_id = %container_id, error = %e, "Failed to tear down container");
            continue;
        }

        running_containers.lock().await.remove(&workload_id);
        workload_phases.lock().await.remove(&workload_id);
        restart_counts.lock().await.remove(&workload_id);
        vm_workload_ids.lock().await.remove(&workload_id);

        let dns_entry = service_dns_registry.lock().await.remove(&workload_id);
        if let Some((resource_group_id, service_name)) = dns_entry {
            if let Err(e) = rg_dns_registry
                .remove(&resource_group_id, &service_name)
                .await
            {
                warn!(workload_id = %workload_id, error = %e, "Failed to remove service dns record");
            }
        }
    }
}

async fn should_restart_after_crash(
    docker: &dyn runtime::Runtime,
    workload: &client::AssignedWorkload,
    container_id: &str,
    restart_counts: &Arc<Mutex<HashMap<String, u32>>>,
) -> bool {
    let status = match docker.inspect_status(container_id).await {
        Ok(s) => s,
        Err(e) => {
            warn!(workload_id = %workload.id, container_id = %container_id, error = %e, "Failed to inspect container");
            return false;
        }
    };

    if status != "failed" {
        return false;
    }

    if workload.restart_policy == "never" {
        if let Err(e) = docker.stop_workload(container_id).await {
            warn!(workload_id = %workload.id, container_id = %container_id, error = %e, "Failed to stop container with restart_policy=never");
        }
        return false;
    }

    let mut counts = restart_counts.lock().await;
    let count = counts.entry(workload.id.clone()).or_insert(0);

    if let Some(max) = workload.max_restarts {
        if *count as i32 >= max {
            warn!(workload_id = %workload.id, restart_count = *count, max_restarts = max, "Crash-loop limit reached, stopping container");
            drop(counts);
            if let Err(e) = docker.stop_workload(container_id).await {
                warn!(workload_id = %workload.id, container_id = %container_id, error = %e, "Failed to stop container after crash-loop limit");
            }
            return false;
        }
    }

    *count += 1;
    info!(workload_id = %workload.id, restart_count = *count, "Container crashed, scheduling restart");
    true
}

#[allow(clippy::too_many_arguments)]
async fn process_workloads(
    client: &client::ApiClient,
    api_key: &str,
    firecracker: &Arc<firecracker::runtime::FirecrackerRuntime>,
    qemu: &Arc<qemu::runtime::QemuRuntime>,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
    vm_workload_ids: &Arc<Mutex<std::collections::HashSet<String>>>,
    workload_phases: &Arc<Mutex<HashMap<String, String>>>,
    mounted_volumes: &Arc<Mutex<HashMap<String, String>>>,
    restart_counts: &Arc<Mutex<HashMap<String, u32>>>,
    service_dns_registry: &Arc<Mutex<HashMap<String, (String, String)>>>,
    rg_dns_registry: &rg_dns::RgDnsRegistry,
) -> Vec<String> {
    let workloads = match client.fetch_assigned_workloads(api_key).await {
        Ok(w) => w,
        Err(e) => {
            warn!(error = %e, "Failed to fetch assigned workloads");
            return Vec::new();
        }
    };

    let resource_group_ids: Vec<String> = {
        let mut ids: Vec<String> = workloads
            .iter()
            .filter_map(|w| w.resource_group_id.clone())
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    };

    let recovered_microvms = firecracker.reconcile_once().await;
    let recovered_vms = qemu.reconcile_once().await;
    if !recovered_microvms.is_empty() || !recovered_vms.is_empty() {
        let mut containers = running_containers.lock().await;
        for (workload_id, handle) in recovered_microvms.into_iter().chain(recovered_vms) {
            containers.insert(workload_id, handle);
        }
    }

    {
        let mut vm_ids = vm_workload_ids.lock().await;
        vm_ids.clear();
        vm_ids.extend(
            workloads
                .iter()
                .filter(|w| w.runtime_class == "vm")
                .map(|w| w.id.clone()),
        );
    }

    reap_stale_containers(
        firecracker.as_ref(),
        qemu.as_ref(),
        vm_workload_ids,
        running_containers,
        workload_phases,
        restart_counts,
        service_dns_registry,
        rg_dns_registry,
        &workloads,
    )
    .await;

    for workload in workloads {
        let workload_id = workload.id.clone();
        let runtime: &dyn runtime::Runtime = if workload.runtime_class == "vm" {
            qemu.as_ref()
        } else {
            firecracker.as_ref()
        };

        let restart_fulfilled = start_or_restart_workload(
            runtime,
            workload,
            running_containers,
            workload_phases,
            mounted_volumes,
            restart_counts,
            service_dns_registry,
            rg_dns_registry,
        )
        .await;

        if restart_fulfilled {
            if let Err(e) = client.ack_workload_restart(api_key, &workload_id).await {
                warn!(workload_id = %workload_id, error = %e, "Failed to ack workload restart");
            }
        }
    }

    resource_group_ids
}

#[allow(clippy::too_many_arguments)]
async fn start_or_restart_workload(
    runtime: &dyn runtime::Runtime,
    workload: client::AssignedWorkload,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
    workload_phases: &Arc<Mutex<HashMap<String, String>>>,
    mounted_volumes: &Arc<Mutex<HashMap<String, String>>>,
    restart_counts: &Arc<Mutex<HashMap<String, u32>>>,
    service_dns_registry: &Arc<Mutex<HashMap<String, (String, String)>>>,
    rg_dns_registry: &rg_dns::RgDnsRegistry,
) -> bool {
    let existing_container_id = running_containers.lock().await.get(&workload.id).cloned();
    let restart_requested = workload.restart_requested;

    if let Some(container_id) = existing_container_id {
        let should_restart = restart_requested
            || should_restart_after_crash(runtime, &workload, &container_id, restart_counts).await;
        if !should_restart {
            return false;
        }
        if let Err(e) = runtime.stop_workload(&container_id).await {
            warn!(workload_id = %workload.id, container_id = %container_id, error = %e, "Failed to stop container for restart");
            return false;
        }
        running_containers.lock().await.remove(&workload.id);
    }

    info!(workload_id = %workload.id, image = %workload.image, "Starting workload");

    workload_phases
        .lock()
        .await
        .insert(workload.id.clone(), "pulling".to_string());

    if let Err(e) = runtime.pull_image(&workload.image).await {
        warn!(workload_id = %workload.id, error = %e, "Failed to pull image");
        workload_phases.lock().await.remove(&workload.id);
        return false;
    }

    if let Some(ref mounts) = workload.volume_mounts {
        let locked = mounted_volumes.lock().await;
        let all_ready = mounts.iter().all(|m| locked.contains_key(&m.volume_id));
        drop(locked);
        if !all_ready {
            info!(workload_id = %workload.id, "Waiting for volumes to be mounted, deferring workload");
            return false;
        }
    }

    if let Some(max) = workload.max_restarts {
        let count = *restart_counts.lock().await.get(&workload.id).unwrap_or(&0);
        if count as i32 >= max {
            workload_phases
                .lock()
                .await
                .insert(workload.id.clone(), "failed".to_string());
            return false;
        }
    }

    workload_phases
        .lock()
        .await
        .insert(workload.id.clone(), "creating".to_string());

    let volume_devices = mounted_volumes.lock().await.clone();

    let vm_spec = if workload.runtime_class == "vm" {
        Some(spec::VmSpec {
            boot_disk_path: qemu::runtime::boot_disk_path(&workload.id),
            boot_disk_size_bytes: workload.disk_bytes,
            iso_url: Some(workload.image.clone()),
        })
    } else {
        None
    };

    let spec = spec::WorkloadSpec {
        workload_id: workload.id.clone(),
        image: workload.image.clone(),
        cpu_millicores: workload.cpu_millicores,
        memory_bytes: workload.memory_bytes,
        env_vars: workload.env_vars,
        ports: workload.ports,
        volume_mounts: workload.volume_mounts.map(|mounts| {
            mounts
                .into_iter()
                .filter_map(|m| {
                    let device_path = volume_devices.get(&m.volume_id)?.clone();
                    Some(spec::VolumeMount {
                        volume_id: m.volume_id,
                        mount_path: m.mount_path,
                        device_path,
                    })
                })
                .collect()
        }),
        service_name: workload.service_name,
        resource_group_id: workload.resource_group_id,
        resource_group_cidr: workload.resource_group_cidr,
        vm_spec,
    };

    match runtime.start_workload(&spec).await {
        Ok(container_id) => {
            workload_phases
                .lock()
                .await
                .insert(workload.id.clone(), "starting".to_string());
            running_containers
                .lock()
                .await
                .insert(workload.id.clone(), container_id.clone());
            info!(
                workload_id = %workload.id,
                container_id = %container_id,
                "Workload started"
            );

            if let (Some(resource_group_id), Some(service_name)) =
                (&spec.resource_group_id, &spec.service_name)
            {
                service_dns_registry.lock().await.insert(
                    workload.id.clone(),
                    (resource_group_id.clone(), service_name.clone()),
                );
                register_service_dns(
                    rg_dns_registry,
                    runtime,
                    &container_id,
                    resource_group_id,
                    service_name,
                )
                .await;
            }

            restart_requested
        }
        Err(e) => {
            let mut counts = restart_counts.lock().await;
            let count = counts.entry(workload.id.clone()).or_insert(0);
            *count += 1;
            warn!(workload_id = %workload.id, error = ?e, attempt = *count, "Failed to start workload");
            workload_phases
                .lock()
                .await
                .insert(workload.id.clone(), "failed".to_string());
            false
        }
    }
}

async fn register_service_dns(
    rg_dns_registry: &rg_dns::RgDnsRegistry,
    runtime: &dyn runtime::Runtime,
    container_id: &str,
    resource_group_id: &str,
    service_name: &str,
) {
    let network_name = spec::rg_network_name(resource_group_id);

    let ip_address = match runtime
        .service_network_ip(container_id, &network_name)
        .await
    {
        Ok(Some(ip)) => ip,
        Ok(None) => {
            warn!(container_id = %container_id, network = %network_name, "No network ip found for service dns registration");
            return;
        }
        Err(e) => {
            warn!(container_id = %container_id, error = %e, "Failed to inspect network ip for service dns registration");
            return;
        }
    };

    if let Err(e) = rg_dns_registry
        .upsert(resource_group_id, service_name, &ip_address)
        .await
    {
        warn!(resource_group_id = %resource_group_id, service_name = %service_name, error = %e, "Failed to write service dns record");
    }
}

async fn sync_wireguard_peers(
    client: &client::ApiClient,
    api_key: &str,
    agent_id: uuid::Uuid,
    resource_group_ids: &[String],
) {
    for resource_group_id in resource_group_ids {
        let peers = match client
            .fetch_resource_group_peers(api_key, resource_group_id)
            .await
        {
            Ok(p) => p,
            Err(e) => {
                warn!(resource_group_id = %resource_group_id, error = %e, "Failed to fetch resource group peers");
                continue;
            }
        };

        let agent_id_str = agent_id.to_string();
        let wg_peers: Vec<wireguard::Peer> = peers
            .into_iter()
            .filter(|p| p.agent_id != agent_id_str)
            .map(|p| wireguard::Peer {
                public_key: p.wg_public_key,
                endpoint: Some(p.wg_endpoint),
                allowed_ips: format!("{}/32", p.wg_tunnel_ip),
            })
            .collect();

        if wg_peers.is_empty() {
            continue;
        }

        let iface = wireguard::rg_interface_name(resource_group_id);
        if let Err(e) = wireguard::set_peers(&iface, &wg_peers).await {
            warn!(resource_group_id = %resource_group_id, iface = %iface, error = %e, "Failed to sync WireGuard peers");
        }
    }
}

async fn sync_vpn_peers(client: &client::ApiClient, api_key: &str, resource_group_ids: &[String]) {
    let mut wg_peers: Vec<wireguard::Peer> = Vec::new();

    for resource_group_id in resource_group_ids {
        match client
            .fetch_resource_group_vpn_peers(api_key, resource_group_id)
            .await
        {
            Ok(peers) => {
                wg_peers.extend(peers.into_iter().map(|p| wireguard::Peer {
                    public_key: p.client_public_key,
                    endpoint: None,
                    allowed_ips: format!("{}/32", p.client_tunnel_ip),
                }));
            }
            Err(e) => {
                warn!(resource_group_id = %resource_group_id, error = %e, "Failed to fetch resource group vpn peers");
            }
        }
    }

    if let Err(e) = wireguard::reconcile_peers(wireguard::MGMT_INTERFACE_NAME, &wg_peers).await {
        warn!(error = %e, "Failed to sync VPN client peers");
    }
}

async fn cleanup_stale_resource_groups(
    client: &client::ApiClient,
    api_key: &str,
    firecracker: &firecracker::runtime::FirecrackerRuntime,
    qemu: &qemu::runtime::QemuRuntime,
) {
    let active_ids = match client.fetch_active_resource_group_ids(api_key).await {
        Ok(ids) => ids,
        Err(e) => {
            warn!(error = %e, "Failed to fetch active resource group ids");
            return;
        }
    };

    let local_ids = match rg_network::list_rg_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            warn!(error = %e, "Failed to list local resource group bridges");
            return;
        }
    };

    for local_id in local_ids {
        if active_ids.iter().any(|id| id == &local_id) {
            continue;
        }

        info!(resource_group_id = %local_id, "Tearing down stale resource group network");

        if let Err(e) = firecracker.teardown_rg_network(&local_id).await {
            warn!(resource_group_id = %local_id, error = %e, "Failed to tear down stale firecracker resource group network");
        }
        if let Err(e) = qemu.teardown_rg_network(&local_id).await {
            warn!(resource_group_id = %local_id, error = %e, "Failed to tear down stale qemu resource group network");
        }
    }
}

async fn build_container_statuses(
    firecracker: &firecracker::runtime::FirecrackerRuntime,
    qemu: &qemu::runtime::QemuRuntime,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
    vm_workload_ids: &Arc<Mutex<std::collections::HashSet<String>>>,
    workload_phases: &Arc<Mutex<HashMap<String, String>>>,
) -> Vec<client::ContainerStatus> {
    let containers = running_containers.lock().await.clone();
    let vm_ids = vm_workload_ids.lock().await.clone();
    let mut statuses = Vec::with_capacity(containers.len());

    for (workload_id, container_id) in containers.iter() {
        let runtime: &dyn runtime::Runtime = if vm_ids.contains(workload_id) {
            qemu
        } else {
            firecracker
        };

        let status = match runtime.inspect_status(container_id).await {
            Ok(s) => s,
            Err(e) => {
                warn!(workload_id = %workload_id, container_id = %container_id, error = %e, "Failed to inspect workload");
                "failed".to_string()
            }
        };

        if status != "creating" {
            workload_phases.lock().await.remove(workload_id);
        }

        statuses.push(client::ContainerStatus {
            workload_id: workload_id.clone(),
            container_id: container_id.clone(),
            status,
        });
    }

    let phases = workload_phases.lock().await;
    for (workload_id, phase) in phases.iter() {
        if !containers.contains_key(workload_id) {
            statuses.push(client::ContainerStatus {
                workload_id: workload_id.clone(),
                container_id: String::new(),
                status: phase.clone(),
            });
        }
    }

    statuses
}

async fn push_workload_stats(
    client: &client::ApiClient,
    api_key: &str,
    firecracker: &firecracker::runtime::FirecrackerRuntime,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
) {
    let containers = running_containers.lock().await.clone();
    if containers.is_empty() {
        return;
    }

    let mut stats = Vec::with_capacity(containers.len());
    for (workload_id, container_id) in containers.iter() {
        match firecracker.stats(container_id).await {
            Ok(s) => stats.push(client::WorkloadStatsUpdate {
                workload_id: workload_id.clone(),
                cpu_usage_percent: s.cpu_usage_percent,
                memory_usage_bytes: s.memory_usage_bytes,
                network_rx_bytes: s.network_rx_bytes,
                network_tx_bytes: s.network_tx_bytes,
            }),
            Err(e) => {
                warn!(workload_id = %workload_id, container_id = %container_id, error = %e, "Failed to collect container stats");
            }
        }
    }

    if let Err(e) = client.push_workload_stats(api_key, stats).await {
        warn!(error = %e, "Failed to push workload stats");
    }
}

const MGMT_WG_PORT: u16 = 51820;

fn detect_wg_endpoint() -> Option<String> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    let local_ip = socket.local_addr().ok()?.ip();
    Some(format!("{}:{}", local_ip, MGMT_WG_PORT))
}
