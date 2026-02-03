use bollard::container::{RestartContainerOptions, StartContainerOptions, StopContainerOptions};
use bollard::Docker;
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DockerError {
    #[error("Docker error: {0}")]
    BollardError(#[from] bollard::errors::Error),
    #[error("Container not found: {0}")]
    ContainerNotFound(String),
    #[error("Docker service not available")]
    NotAvailable,
}

#[derive(Clone)]
pub struct DockerService {
    client: Docker,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub image: String,
    pub state: String,
    pub status: String,
}

impl DockerService {
    pub fn new() -> Result<Self, DockerError> {
        // Try multiple socket locations
        let socket_paths = vec![
            "/var/run/docker.sock",
            "/run/docker.sock",
            "unix:///var/run/docker.sock",
            "unix:///run/docker.sock",
        ];

        // First try default socket connection
        if let Ok(client) = Docker::connect_with_socket_defaults() {
            tracing::info!("Connected to Docker via default socket");
            return Ok(Self { client });
        }

        // Try each socket path explicitly
        for socket_path in socket_paths {
            if let Ok(client) =
                Docker::connect_with_unix(socket_path, 120, bollard::API_DEFAULT_VERSION)
            {
                tracing::info!("Connected to Docker via socket: {}", socket_path);
                return Ok(Self { client });
            }
        }

        // Check if docker socket exists but we don't have permissions
        if std::path::Path::new("/var/run/docker.sock").exists() {
            tracing::error!(
                "Docker socket exists at /var/run/docker.sock but connection failed. \
                This is likely a permissions issue. Please add the service user to the 'docker' group: \
                sudo usermod -aG docker csf-core"
            );
        } else {
            tracing::error!("Docker socket not found. Is Docker installed and running?");
        }

        Err(DockerError::NotAvailable)
    }

    /// Check if Docker is available
    pub async fn is_available(&self) -> bool {
        self.client.ping().await.is_ok()
    }

    /// Start a container by ID or name
    pub async fn start_container(&self, id: &str) -> Result<(), DockerError> {
        self.client
            .start_container(id, None::<StartContainerOptions<String>>)
            .await?;
        Ok(())
    }

    /// Stop a container by ID or name
    pub async fn stop_container(&self, id: &str) -> Result<(), DockerError> {
        self.client
            .stop_container(id, None::<StopContainerOptions>)
            .await?;
        Ok(())
    }

    /// Restart a container by ID or name
    pub async fn restart_container(&self, id: &str) -> Result<(), DockerError> {
        self.client
            .restart_container(id, None::<RestartContainerOptions>)
            .await?;
        Ok(())
    }

    /// Get container info by ID or name
    pub async fn inspect_container(&self, id: &str) -> Result<ContainerInfo, DockerError> {
        let info = self.client.inspect_container(id, None).await?;

        let state = info
            .state
            .as_ref()
            .and_then(|s| s.status.as_ref())
            .map(|s| format!("{:?}", s).to_lowercase())
            .unwrap_or_else(|| "unknown".to_string());

        let status = info
            .state
            .as_ref()
            .and_then(|s| s.status.as_ref())
            .map(|s| format!("{:?}", s))
            .unwrap_or_else(|| "Unknown".to_string());

        Ok(ContainerInfo {
            id: info.id.unwrap_or_default(),
            name: info
                .name
                .unwrap_or_default()
                .trim_start_matches('/')
                .to_string(),
            image: info
                .config
                .and_then(|c| c.image)
                .unwrap_or_else(|| "unknown".to_string()),
            state,
            status,
        })
    }

    /// List all containers
    pub async fn list_containers(&self, all: bool) -> Result<Vec<ContainerInfo>, DockerError> {
        use bollard::container::ListContainersOptions;
        use std::collections::HashMap;

        let mut filters = HashMap::new();
        if !all {
            filters.insert("status".to_string(), vec!["running".to_string()]);
        }

        let options = Some(ListContainersOptions {
            all,
            filters,
            ..Default::default()
        });

        let containers = self.client.list_containers(options).await?;

        let result = containers
            .into_iter()
            .map(|c| ContainerInfo {
                id: c.id.unwrap_or_default(),
                name: c
                    .names
                    .and_then(|names| names.first().map(|n| n.trim_start_matches('/').to_string()))
                    .unwrap_or_default(),
                image: c.image.unwrap_or_default(),
                state: c.state.unwrap_or_else(|| "unknown".to_string()),
                status: c.status.unwrap_or_else(|| "Unknown".to_string()),
            })
            .collect();

        Ok(result)
    }

    /// Pull a Docker image
    pub async fn pull_image(&self, image: &str) -> Result<(), DockerError> {
        use bollard::image::CreateImageOptions;
        use futures_util::stream::StreamExt;

        let (image_name, tag) = if image.contains(':') {
            let parts: Vec<&str> = image.split(':').collect();
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (image.to_string(), "latest".to_string())
        };

        let options = Some(CreateImageOptions {
            from_image: image_name.as_str(),
            tag: tag.as_str(),
            ..Default::default()
        });

        let mut stream = self.client.create_image(options, None, None);

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        tracing::debug!("Pull image: {}", status);
                    }
                    if let Some(error) = info.error {
                        tracing::error!("Pull image error: {}", error);
                        return Err(DockerError::BollardError(
                            bollard::errors::Error::DockerResponseServerError {
                                status_code: 500,
                                message: error,
                            },
                        ));
                    }
                }
                Err(e) => return Err(DockerError::BollardError(e)),
            }
        }

        Ok(())
    }

    /// Create and start a new container
    pub async fn create_and_start_container(
        &self,
        name: &str,
        image: &str,
        config: &serde_json::Value,
    ) -> Result<String, DockerError> {
        use bollard::container::{Config, CreateContainerOptions};
        use std::collections::HashMap;

        // Parse configuration
        let mut port_bindings = HashMap::new();
        let mut exposed_ports = HashMap::new();

        if let Some(ports) = config.get("ports").and_then(|p| p.as_array()) {
            for port in ports {
                if let (Some(container_port), host_port) = (
                    port.get("container").and_then(|p| p.as_u64()),
                    port.get("host").and_then(|p| p.as_u64()),
                ) {
                    let container_port_str = format!("{}/tcp", container_port);
                    exposed_ports.insert(container_port_str.clone(), HashMap::new());

                    if let Some(hp) = host_port {
                        port_bindings.insert(
                            container_port_str,
                            Some(vec![bollard::service::PortBinding {
                                host_ip: Some("0.0.0.0".to_string()),
                                host_port: Some(hp.to_string()),
                            }]),
                        );
                    }
                }
            }
        }

        let mut env_vars = Vec::new();
        if let Some(env) = config.get("environment").and_then(|e| e.as_object()) {
            for (key, value) in env {
                if let Some(val_str) = value.as_str() {
                    env_vars.push(format!("{}={}", key, val_str));
                }
            }
        }

        let mut binds = Vec::new();
        if let Some(volumes) = config.get("volumes").and_then(|v| v.as_array()) {
            for volume in volumes {
                if let (Some(host), Some(container)) = (
                    volume.get("host").and_then(|h| h.as_str()),
                    volume.get("container").and_then(|c| c.as_str()),
                ) {
                    binds.push(format!("{}:{}", host, container));
                }
            }
        }

        let _restart_policy = config
            .get("restart_policy")
            .and_then(|r| r.as_str())
            .unwrap_or("unless-stopped");

        let container_config = Config {
            image: Some(image.to_string()),
            exposed_ports: if exposed_ports.is_empty() {
                None
            } else {
                Some(exposed_ports)
            },
            env: if env_vars.is_empty() {
                None
            } else {
                Some(env_vars)
            },
            host_config: Some(bollard::service::HostConfig {
                port_bindings: if port_bindings.is_empty() {
                    None
                } else {
                    Some(port_bindings)
                },
                binds: if binds.is_empty() { None } else { Some(binds) },
                restart_policy: Some(bollard::service::RestartPolicy {
                    name: Some(bollard::service::RestartPolicyNameEnum::UNLESS_STOPPED),
                    maximum_retry_count: None,
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        let options = CreateContainerOptions {
            name,
            platform: None,
        };

        // Create container
        let response = self
            .client
            .create_container(Some(options), container_config)
            .await?;

        let container_id = response.id;

        // Start container
        self.start_container(&container_id).await?;

        Ok(container_id)
    }

    /// Get container logs
    pub async fn get_container_logs(
        &self,
        id: &str,
        tail: Option<usize>,
    ) -> Result<String, DockerError> {
        use bollard::container::LogsOptions;
        use futures_util::stream::StreamExt;

        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            follow: false,
            tail: tail.unwrap_or(100).to_string(),
            ..Default::default()
        };

        let mut stream = self.client.logs(id, Some(options));
        let mut logs = String::new();

        while let Some(log) = stream.next().await {
            match log {
                Ok(output) => {
                    logs.push_str(&output.to_string());
                }
                Err(e) => {
                    tracing::error!("Error reading logs: {}", e);
                    break;
                }
            }
        }

        Ok(logs)
    }

    /// Execute a command in a container
    pub async fn exec_in_container(
        &self,
        id: &str,
        cmd: Vec<String>,
    ) -> Result<String, DockerError> {
        use bollard::exec::{CreateExecOptions, StartExecResults};
        use futures_util::stream::StreamExt;

        // Create exec instance
        let exec_config = CreateExecOptions {
            cmd: Some(cmd),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            ..Default::default()
        };

        let exec = self.client.create_exec(id, exec_config).await?;

        // Start exec
        let start_result = self
            .client
            .start_exec(&exec.id, None::<bollard::exec::StartExecOptions>)
            .await?;

        let mut output = String::new();

        match start_result {
            StartExecResults::Attached {
                output: mut stream,
                input: _,
            } => {
                while let Some(msg) = stream.next().await {
                    match msg {
                        Ok(log_output) => {
                            output.push_str(&log_output.to_string());
                        }
                        Err(e) => {
                            tracing::error!("Error reading exec output: {}", e);
                            break;
                        }
                    }
                }
            }
            StartExecResults::Detached => {
                tracing::warn!("Exec started in detached mode");
            }
        }

        Ok(output)
    }
}
