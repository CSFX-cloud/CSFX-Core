use anyhow::{Context, Result};
use bollard::container::{Config, CreateContainerOptions, StartContainerOptions};
use bollard::image::CreateImageOptions;
use bollard::models::HostConfig;
use bollard::Docker;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub host_port: u16,
    pub container_port: u16,
    pub protocol: Option<String>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct WorkloadSpec {
    pub workload_id: String,
    pub name: String,
    pub image: String,
    pub env_vars: Option<HashMap<String, String>>,
    pub ports: Option<Vec<PortMapping>>,
}

pub struct DockerManager {
    docker: Docker,
}

impl DockerManager {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_unix_defaults()
            .context("Failed to connect to Docker socket")?;
        Ok(Self { docker })
    }

    pub async fn pull_image(&self, image: &str) -> Result<()> {
        info!(image = %image, "Pulling image");

        let options = CreateImageOptions {
            from_image: image,
            ..Default::default()
        };

        let mut stream = self.docker.create_image(Some(options), None, None);

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        if status != "Pulling fs layer"
                            && status != "Waiting"
                            && status != "Downloading"
                            && status != "Verifying Checksum"
                            && status != "Extracting"
                        {
                            info!(image = %image, status = %status, "Pull progress");
                        }
                    }
                }
                Err(e) => {
                    warn!(image = %image, error = %e, "Pull stream error");
                }
            }
        }

        info!(image = %image, "Image pull complete");
        Ok(())
    }

    pub async fn start_container(&self, spec: &WorkloadSpec) -> Result<String> {
        let container_name = format!("csf-{}", spec.workload_id);

        let env: Option<Vec<String>> = spec.env_vars.as_ref().map(|vars| {
            vars.iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect()
        });

        let (port_bindings, exposed_ports) = build_port_config(spec.ports.as_deref());

        let host_config = HostConfig {
            port_bindings: if port_bindings.is_empty() {
                None
            } else {
                Some(port_bindings)
            },
            ..Default::default()
        };

        let config = Config {
            image: Some(spec.image.clone()),
            env,
            exposed_ports: if exposed_ports.is_empty() {
                None
            } else {
                Some(exposed_ports)
            },
            host_config: Some(host_config),
            labels: Some(HashMap::from([
                ("csf.workload_id".to_string(), spec.workload_id.clone()),
                ("csf.managed".to_string(), "true".to_string()),
            ])),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name: container_name.clone(),
            platform: None,
        };

        let container = self
            .docker
            .create_container(Some(options), config)
            .await
            .context("Failed to create container")?;

        self.docker
            .start_container(&container.id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;

        info!(
            workload_id = %spec.workload_id,
            container_id = %container.id,
            "Container started"
        );

        Ok(container.id)
    }

}

#[allow(clippy::type_complexity)]
fn build_port_config(
    ports: Option<&[PortMapping]>,
) -> (
    HashMap<String, Option<Vec<bollard::models::PortBinding>>>,
    HashMap<String, HashMap<(), ()>>,
) {
    let mut port_bindings: HashMap<String, Option<Vec<bollard::models::PortBinding>>> =
        HashMap::new();
    let mut exposed_ports: HashMap<String, HashMap<(), ()>> = HashMap::new();

    if let Some(ports) = ports {
        for p in ports {
            let proto = p.protocol.as_deref().unwrap_or("tcp");
            let container_key = format!("{}/{}", p.container_port, proto);

            port_bindings.insert(
                container_key.clone(),
                Some(vec![bollard::models::PortBinding {
                    host_ip: Some("0.0.0.0".to_string()),
                    host_port: Some(p.host_port.to_string()),
                }]),
            );

            exposed_ports.insert(container_key, HashMap::new());
        }
    }

    (port_bindings, exposed_ports)
}
