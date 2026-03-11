mod client;
mod config;
mod docker;
mod pki;
mod rbd;
mod system;

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tracing::{error, info, warn};

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_target(false)
        .init();

    info!(version = env!("CARGO_PKG_VERSION"), "csf-agent starting");

    let gateway_url = std::env::var("CSF_GATEWAY_URL")
        .context("CSF_GATEWAY_URL environment variable is required")?;

    let heartbeat_interval_secs: u64 = std::env::var("CSF_HEARTBEAT_INTERVAL")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);

    let api_client = client::ApiClient::new(gateway_url.clone())
        .context("Failed to initialize API client")?;

    let agent_pki = pki::AgentPki::load_or_generate()
        .context("Failed to initialize PKI")?;

    let (agent_id, api_key) = if config::is_registered() {
        info!("Existing registration found, loading credentials");
        let cfg = config::load_config().context("Failed to load daemon config")?;
        let creds = config::load_credentials().context("Failed to load credentials")?;
        (cfg.agent_id, creds.api_key)
    } else {
        info!("No registration found, starting registration");
        perform_registration(&api_client, &gateway_url, heartbeat_interval_secs, &agent_pki)
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

    let docker_manager = match docker::DockerManager::new() {
        Ok(dm) => {
            info!("Docker socket connected");
            Some(Arc::new(dm))
        }
        Err(e) => {
            warn!(error = %e, "Docker unavailable, container management disabled");
            None
        }
    };

    let running_containers: Arc<Mutex<HashMap<String, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    let mounted_volumes: Arc<Mutex<HashMap<String, String>>> =
        Arc::new(Mutex::new(HashMap::new()));

    run_heartbeat_loop(
        &api_client,
        agent_id,
        &api_key,
        heartbeat_interval_secs,
        docker_manager,
        running_containers,
        mounted_volumes,
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
    let token = match std::env::var("CSF_REGISTRATION_TOKEN") {
        Ok(t) => t,
        Err(_) => {
            info!("CSF_REGISTRATION_TOKEN not set, fetching bootstrap token from gateway");
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
        pki::AgentPki::save_certificate(cert_pem, ca_pem)
            .context("Failed to save certificate")?;
        info!("PKI: certificate received and stored");
    } else {
        warn!("Registry did not issue a certificate during registration");
    }

    let cfg = config::DaemonConfig {
        gateway_url: gateway_url.to_string(),
        agent_id: resp.agent_id,
        heartbeat_interval_secs,
    };

    config::save_config(&cfg).context("Failed to save daemon config")?;
    config::save_credentials(&resp.api_key).context("Failed to save credentials")?;

    info!(agent_id = %resp.agent_id, "Registration successful");

    Ok((resp.agent_id, resp.api_key))
}

async fn run_heartbeat_loop(
    client: &client::ApiClient,
    agent_id: uuid::Uuid,
    api_key: &str,
    interval_secs: u64,
    docker: Option<Arc<docker::DockerManager>>,
    running_containers: Arc<Mutex<HashMap<String, String>>>,
    mounted_volumes: Arc<Mutex<HashMap<String, String>>>,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(interval_secs));
    let mut failure_count: u32 = 0;

    loop {
        tokio::select! {
            _ = interval.tick() => {
                process_volumes(client, agent_id, api_key, &mounted_volumes).await;

                if let Some(ref dm) = docker {
                    process_workloads(client, api_key, dm, &running_containers).await;
                }

                let statuses = build_container_statuses(&running_containers).await;
                let metrics = system::collect_metrics();

                match client.heartbeat(agent_id, api_key, Some(statuses), Some(metrics)).await {
                    Ok(_) => {
                        if failure_count > 0 {
                            info!(agent_id = %agent_id, "Heartbeat recovered after {} failures", failure_count);
                            failure_count = 0;
                        }
                    }
                    Err(e) => {
                        failure_count += 1;
                        warn!(
                            agent_id = %agent_id,
                            failures = failure_count,
                            error = %e,
                            "Heartbeat failed"
                        );
                    }
                }
            }
            _ = tokio::signal::ctrl_c() => {
                info!("Shutdown signal received");
                break;
            }
        }
    }

    if failure_count > 0 {
        error!(failures = failure_count, "Agent shutting down with unresolved heartbeat failures");
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

        let mount_point = rbd::mount_point_for(&volume.id);

        if let Err(e) = rbd::mount(&device, &mount_point).await {
            warn!(volume_id = %volume.id, error = %e, "Failed to mount device");
            let _ = rbd::unmap_device(&device).await;
            continue;
        }

        mounted_volumes
            .lock()
            .await
            .insert(volume.id.clone(), device.clone());

        info!(
            volume_id = %volume.id,
            device = %device,
            mount_point = %mount_point,
            "Volume mounted"
        );
    }
}

async fn process_workloads(
    client: &client::ApiClient,
    api_key: &str,
    docker: &docker::DockerManager,
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
) {
    let workloads = match client.fetch_assigned_workloads(api_key).await {
        Ok(w) => w,
        Err(e) => {
            warn!(error = %e, "Failed to fetch assigned workloads");
            return;
        }
    };

    for workload in workloads {
        let already_running = running_containers
            .lock()
            .await
            .contains_key(&workload.id);

        if already_running {
            continue;
        }

        info!(workload_id = %workload.id, image = %workload.image, "Starting workload");

        if let Err(e) = docker.pull_image(&workload.image).await {
            warn!(workload_id = %workload.id, error = %e, "Failed to pull image");
            continue;
        }

        let spec = docker::WorkloadSpec {
            workload_id: workload.id.clone(),
            name: workload.name.clone(),
            image: workload.image.clone(),
            env_vars: workload.env_vars,
            ports: workload.ports,
        };

        match docker.start_container(&spec).await {
            Ok(container_id) => {
                running_containers
                    .lock()
                    .await
                    .insert(workload.id.clone(), container_id.clone());
                info!(
                    workload_id = %workload.id,
                    container_id = %container_id,
                    "Workload started"
                );
            }
            Err(e) => {
                warn!(workload_id = %workload.id, error = %e, "Failed to start container");
            }
        }
    }
}

async fn build_container_statuses(
    running_containers: &Arc<Mutex<HashMap<String, String>>>,
) -> Vec<client::ContainerStatus> {
    running_containers
        .lock()
        .await
        .iter()
        .map(|(workload_id, container_id)| client::ContainerStatus {
            workload_id: workload_id.clone(),
            container_id: container_id.clone(),
            status: "running".to_string(),
        })
        .collect()
}
