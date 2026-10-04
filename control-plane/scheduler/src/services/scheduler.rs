use chrono::Utc;
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::compose::{ComposeServiceSpec, CreateStackRequest, CreateStackResponse};
use crate::models::workload::{
    AgentResources, CreateWorkloadRequest, CreateWorkloadResponse, UpdateWorkloadRequest,
    WorkloadStatus,
};
use crate::services::compose_parser::{self, ComposeParseError};
use crate::services::etcd::{delete_placement, put_placement, PlacementRecord};

pub struct SchedulerService {
    db: DatabaseConnection,
    etcd: Arc<Mutex<EtcdClient>>,
}

impl SchedulerService {
    pub fn new(db: DatabaseConnection, etcd: Arc<Mutex<EtcdClient>>) -> Self {
        Self { db, etcd }
    }

    pub async fn schedule(
        &self,
        mut req: CreateWorkloadRequest,
    ) -> Result<CreateWorkloadResponse, String> {
        if let Some(bindings) = req.bucket_bindings.clone() {
            if !bindings.is_empty() {
                let placeholder_id = Uuid::new_v4();
                let bucket_env = crate::services::bucket_bindings::resolve_env_vars(
                    placeholder_id,
                    req.resource_group_id,
                    &bindings,
                )
                .await;
                req.env_vars
                    .get_or_insert_with(std::collections::HashMap::new)
                    .extend(bucket_env);
            }
        }

        let workload = crate::db::workloads::create(&self.db, &req)
            .await
            .map_err(|e| format!("Failed to persist workload: {}", e))?;

        let mut agents = crate::db::agents::get_online_agents_with_resources(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        for agent in agents.iter_mut() {
            let (reserved_cpu, reserved_mem, reserved_disk) =
                crate::db::agents::get_assigned_workload_resources(&self.db, agent.agent_id)
                    .await
                    .map_err(|e| format!("Failed to fetch reserved resources: {}", e))?;

            agent.free_cpu_millicores -= reserved_cpu;
            agent.free_memory_bytes -= reserved_mem;
            agent.free_disk_bytes -= reserved_disk;
        }

        let volume_pinned_agent = self.resolve_volume_affinity(&req).await?;

        let placement = if let Some(required_agent) = volume_pinned_agent {
            agents.retain(|a| a.agent_id == required_agent);
            self.first_fit(&req, &agents)
        } else if let Some(resource_group_id) = req.resource_group_id {
            let preferred = self
                .preferred_resource_group_agents(resource_group_id, &agents)
                .await?;
            self.first_fit(&req, &preferred)
                .or_else(|| self.first_fit(&req, &agents))
        } else {
            self.first_fit(&req, &agents)
        };

        match placement {
            Some(agent_id) => {
                crate::db::workloads::assign(&self.db, workload.id, agent_id)
                    .await
                    .map_err(|e| format!("Failed to assign workload: {}", e))?;

                if let Some(mut ports) = req.ports.clone() {
                    crate::services::port_allocator::allocate_node_ports(
                        &self.db,
                        agent_id,
                        workload.id,
                        &mut ports,
                    )
                    .await?;
                    let ports_json = crate::services::port_allocator::ports_to_json(&ports)?;
                    crate::db::workloads::update_ports(&self.db, workload.id, ports_json)
                        .await
                        .map_err(|e| format!("Failed to persist allocated node ports: {}", e))?;
                }

                let record = PlacementRecord {
                    workload_id: workload.id,
                    agent_id,
                    image: req.image.clone(),
                    cpu_millicores: req.cpu_millicores,
                    memory_bytes: req.memory_bytes,
                    disk_bytes: req.disk_bytes,
                    scheduled_at: Utc::now().to_rfc3339(),
                    stack_id: req.stack_id,
                    service_name: req.service_name.clone(),
                    runtime_class: req.runtime_class.as_str().to_string(),
                };

                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));

                put_placement(&self.etcd, &record).await?;

                crate::log_info!(
                    "scheduler",
                    &format!(
                        "Workload scheduled workload_id={} agent_id={}",
                        workload.id, agent_id
                    )
                );

                Ok(CreateWorkloadResponse {
                    workload_id: workload.id,
                    status: WorkloadStatus::Scheduled,
                    assigned_agent_id: Some(agent_id),
                    message: format!("Workload assigned to agent {}", agent_id),
                })
            }
            None => {
                crate::log_warn!(
                    "scheduler",
                    &format!(
                        "No suitable agent found workload_id={} required_cpu={} required_memory={} required_disk={} requires_kvm={} {}",
                        workload.id,
                        req.cpu_millicores,
                        req.memory_bytes,
                        req.disk_bytes,
                        req.runtime_class.requires_kvm(),
                        describe_agents(&agents)
                    )
                );

                Ok(CreateWorkloadResponse {
                    workload_id: workload.id,
                    status: WorkloadStatus::Pending,
                    assigned_agent_id: None,
                    message: "No agent with sufficient resources available".to_string(),
                })
            }
        }
    }

    pub async fn schedule_stack(
        &self,
        req: CreateStackRequest,
    ) -> Result<CreateStackResponse, String> {
        let services = compose_parser::parse_compose(&req.compose_yaml)
            .map_err(|e| format_compose_error(&e))?;

        let stack = crate::db::workload_stacks::create(
            &self.db,
            req.resource_group_id,
            &req.name,
            &req.compose_yaml,
        )
        .await
        .map_err(|e| format!("Failed to persist stack: {}", e))?;

        let workloads = self
            .place_stack_workloads(stack.id, req.resource_group_id, &services)
            .await?;

        crate::db::workload_stacks::update_status(
            &self.db,
            stack.id,
            if workloads.iter().all(|w| w.assigned_agent_id.is_some()) {
                "active"
            } else {
                "pending"
            },
        )
        .await
        .map_err(|e| format!("Failed to update stack status: {}", e))?;

        Ok(CreateStackResponse {
            stack_id: stack.id,
            workloads,
        })
    }

    pub async fn delete_stack(&self, stack_id: Uuid) -> Result<(), String> {
        let workloads = crate::db::workloads::get_by_stack_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to fetch stack workloads: {}", e))?;

        for workload in &workloads {
            delete_placement(&self.etcd, workload.id).await?;
        }

        crate::db::workloads::delete_by_stack_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to delete stack workloads: {}", e))?;

        crate::db::workload_stacks::delete(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to delete stack: {}", e))?;

        Ok(())
    }

    pub async fn stop_stack(&self, stack_id: Uuid) -> Result<(), String> {
        let workloads = crate::db::workloads::get_by_stack_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to fetch stack workloads: {}", e))?;

        for workload in &workloads {
            let model = crate::db::workloads::set_desired_state(
                &self.db,
                workload.id,
                crate::models::workload::DesiredState::Stopped,
            )
            .await
            .map_err(|e| format!("Failed to stop workload: {}", e))?;
            if let Some(agent_id) = model.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
        }

        Ok(())
    }

    pub async fn restart_stack(&self, stack_id: Uuid) -> Result<(), String> {
        let workloads = crate::db::workloads::get_by_stack_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to fetch stack workloads: {}", e))?;

        for workload in &workloads {
            let model = crate::db::workloads::request_restart(&self.db, workload.id)
                .await
                .map_err(|e| format!("Failed to restart workload: {}", e))?;
            if let Some(agent_id) = model.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
        }

        Ok(())
    }

    pub async fn redeploy_stack(&self, stack_id: Uuid, compose_yaml: &str) -> Result<(), String> {
        let services =
            compose_parser::parse_compose(compose_yaml).map_err(|e| format_compose_error(&e))?;

        let existing = crate::db::workloads::get_by_stack_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to fetch stack workloads: {}", e))?;

        for service in &services {
            let Some(workload) = existing
                .iter()
                .find(|w| w.service_name.as_deref() == Some(service.service_name.as_str()))
            else {
                continue;
            };

            let update = UpdateWorkloadRequest {
                image: Some(service.image.clone()),
                env_vars: Some(service.env_vars.clone().unwrap_or_default()),
                ports: service.ports.clone(),
                restart_policy: None,
                max_restarts: None,
            };

            let model = crate::db::workloads::update_spec(&self.db, workload.id, &update)
                .await
                .map_err(|e| format!("Failed to update workload: {}", e))?;
            if let Some(agent_id) = model.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
        }

        crate::db::workload_stacks::update_compose_source(&self.db, stack_id, compose_yaml)
            .await
            .map_err(|e| format!("Failed to update stack source: {}", e))?;

        Ok(())
    }

    async fn place_stack_workloads(
        &self,
        stack_id: Uuid,
        resource_group_id: Uuid,
        services: &[ComposeServiceSpec],
    ) -> Result<Vec<CreateWorkloadResponse>, String> {
        let mut agents = crate::db::agents::get_online_agents_with_resources(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        for agent in agents.iter_mut() {
            let (reserved_cpu, reserved_mem, reserved_disk) =
                crate::db::agents::get_assigned_workload_resources(&self.db, agent.agent_id)
                    .await
                    .map_err(|e| format!("Failed to fetch reserved resources: {}", e))?;

            agent.free_cpu_millicores -= reserved_cpu;
            agent.free_memory_bytes -= reserved_mem;
            agent.free_disk_bytes -= reserved_disk;
        }

        let total_cpu: i32 = services.iter().map(|s| s.cpu_millicores).sum();
        let total_memory: i64 = services.iter().map(|s| s.memory_bytes).sum();
        let total_disk: i64 = services.iter().map(|s| s.disk_bytes).sum();

        let agent_id =
            self.first_fit_resources(total_cpu, total_memory, total_disk, false, &agents);

        let mut responses = Vec::with_capacity(services.len());
        for service in services {
            let req = CreateWorkloadRequest {
                name: format!("{}-{}", stack_id, service.service_name),
                image: service.image.clone(),
                cpu_millicores: service.cpu_millicores,
                memory_bytes: service.memory_bytes,
                disk_bytes: service.disk_bytes,
                env_vars: service.env_vars.clone(),
                ports: service.ports.clone(),
                volume_mounts: None,
                bucket_bindings: None,
                resource_group_id: Some(resource_group_id),
                stack_id: Some(stack_id),
                service_name: Some(service.service_name.clone()),
                restart_policy: crate::models::workload::RestartPolicy::Always,
                max_restarts: None,
                runtime_class: crate::models::workload::RuntimeClass::Firecracker,
            };

            let workload = crate::db::workloads::create(&self.db, &req)
                .await
                .map_err(|e| format!("Failed to persist workload: {}", e))?;

            responses.push(match agent_id {
                Some(agent_id) => {
                    crate::db::workloads::assign(&self.db, workload.id, agent_id)
                        .await
                        .map_err(|e| format!("Failed to assign workload: {}", e))?;

                    let record = PlacementRecord {
                        workload_id: workload.id,
                        agent_id,
                        image: req.image.clone(),
                        cpu_millicores: req.cpu_millicores,
                        memory_bytes: req.memory_bytes,
                        disk_bytes: req.disk_bytes,
                        scheduled_at: Utc::now().to_rfc3339(),
                        stack_id: Some(stack_id),
                        service_name: Some(service.service_name.clone()),
                        runtime_class: req.runtime_class.as_str().to_string(),
                    };
                    put_placement(&self.etcd, &record).await?;
                    tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));

                    CreateWorkloadResponse {
                        workload_id: workload.id,
                        status: WorkloadStatus::Scheduled,
                        assigned_agent_id: Some(agent_id),
                        message: format!("Workload assigned to agent {}", agent_id),
                    }
                }
                None => CreateWorkloadResponse {
                    workload_id: workload.id,
                    status: WorkloadStatus::Pending,
                    assigned_agent_id: None,
                    message: "No agent with sufficient resources available".to_string(),
                },
            });
        }

        crate::log_info!(
            "scheduler",
            &format!(
                "Stack scheduled stack_id={} agent_id={:?} services={}",
                stack_id,
                agent_id,
                services.len()
            )
        );

        Ok(responses)
    }

    async fn preferred_resource_group_agents(
        &self,
        resource_group_id: Uuid,
        agents: &[AgentResources],
    ) -> Result<Vec<AgentResources>, String> {
        let hosting_agents =
            crate::db::agents::get_agents_hosting_resource_group(&self.db, resource_group_id)
                .await
                .map_err(|e| format!("Failed to resolve resource group affinity: {}", e))?;

        Ok(agents
            .iter()
            .filter(|a| hosting_agents.contains(&a.agent_id))
            .cloned()
            .collect())
    }

    async fn resolve_volume_affinity(
        &self,
        req: &CreateWorkloadRequest,
    ) -> Result<Option<Uuid>, String> {
        let mounts = match &req.volume_mounts {
            Some(m) if !m.is_empty() => m,
            _ => return Ok(None),
        };

        let mut pinned: Option<Uuid> = None;

        for mount in mounts {
            let agent_id = crate::db::agents::get_volume_agent(&self.db, mount.volume_id)
                .await
                .map_err(|e| format!("Failed to check volume affinity: {}", e))?;

            if let Some(aid) = agent_id {
                match pinned {
                    None => pinned = Some(aid),
                    Some(existing) if existing != aid => {
                        return Err(format!(
                            "Volume mounts require conflicting agents: {} vs {}",
                            existing, aid
                        ));
                    }
                    _ => {}
                }
            }
        }

        Ok(pinned)
    }

    fn first_fit(&self, req: &CreateWorkloadRequest, agents: &[AgentResources]) -> Option<Uuid> {
        self.first_fit_resources(
            req.cpu_millicores,
            req.memory_bytes,
            req.disk_bytes,
            req.runtime_class.requires_kvm(),
            agents,
        )
    }

    fn first_fit_resources(
        &self,
        cpu_millicores: i32,
        memory_bytes: i64,
        disk_bytes: i64,
        requires_kvm: bool,
        agents: &[AgentResources],
    ) -> Option<Uuid> {
        let mut sorted: Vec<&AgentResources> = agents.iter().collect();
        sorted.sort_by(|a, b| {
            let score_a = a.free_cpu_millicores as i64 + a.free_memory_bytes / (1024 * 1024);
            let score_b = b.free_cpu_millicores as i64 + b.free_memory_bytes / (1024 * 1024);
            score_b.cmp(&score_a)
        });

        sorted
            .into_iter()
            .find(|a| {
                a.free_cpu_millicores >= cpu_millicores
                    && a.free_memory_bytes >= memory_bytes
                    && a.free_disk_bytes >= disk_bytes
                    && (!requires_kvm || a.kvm_capable)
            })
            .map(|a| a.agent_id)
    }

    pub async fn list_workloads(
        &self,
    ) -> Result<Vec<crate::models::workload::WorkloadResponse>, String> {
        crate::db::workloads::get_all(&self.db)
            .await
            .map_err(|e| format!("Failed to list workloads: {}", e))
    }

    pub async fn reschedule_from_agent(
        &self,
        dead_agent_id: Uuid,
        workload_ids: &[Uuid],
    ) -> Result<Vec<CreateWorkloadResponse>, String> {
        let mut agents = crate::db::agents::get_online_agents_with_resources(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        agents.retain(|a| a.agent_id != dead_agent_id);

        for agent in agents.iter_mut() {
            let (reserved_cpu, reserved_mem, reserved_disk) =
                crate::db::agents::get_assigned_workload_resources(&self.db, agent.agent_id)
                    .await
                    .map_err(|e| format!("Failed to fetch reserved resources: {}", e))?;

            agent.free_cpu_millicores -= reserved_cpu;
            agent.free_memory_bytes -= reserved_mem;
            agent.free_disk_bytes -= reserved_disk;
        }

        let mut responses = Vec::with_capacity(workload_ids.len());

        for &workload_id in workload_ids {
            let workload = crate::db::workloads::get_by_id(&self.db, workload_id)
                .await
                .map_err(|e| format!("Failed to fetch workload: {}", e))?
                .ok_or_else(|| format!("Workload {} not found", workload_id))?;

            let runtime_class =
                crate::models::workload::RuntimeClass::from_str(&workload.runtime_class);

            let placed = self.first_fit_resources(
                workload.cpu_millicores,
                workload.memory_bytes,
                workload.disk_bytes,
                runtime_class.requires_kvm(),
                &agents,
            );

            match placed {
                Some(agent_id) => {
                    crate::db::workloads::assign(&self.db, workload_id, agent_id)
                        .await
                        .map_err(|e| format!("Failed to assign workload: {}", e))?;

                    let record = PlacementRecord {
                        workload_id,
                        agent_id,
                        image: workload.image.clone(),
                        cpu_millicores: workload.cpu_millicores,
                        memory_bytes: workload.memory_bytes,
                        disk_bytes: workload.disk_bytes,
                        scheduled_at: Utc::now().to_rfc3339(),
                        stack_id: workload.stack_id,
                        service_name: workload.service_name.clone(),
                        runtime_class: workload.runtime_class.clone(),
                    };
                    put_placement(&self.etcd, &record).await?;
                    tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));

                    if let Some(agent) = agents.iter_mut().find(|a| a.agent_id == agent_id) {
                        agent.free_cpu_millicores -= workload.cpu_millicores;
                        agent.free_memory_bytes -= workload.memory_bytes;
                        agent.free_disk_bytes -= workload.disk_bytes;
                    }

                    crate::log_info!(
                        "scheduler",
                        &format!(
                            "Workload rescheduled workload_id={} from_agent_id={} to_agent_id={}",
                            workload_id, dead_agent_id, agent_id
                        )
                    );

                    responses.push(CreateWorkloadResponse {
                        workload_id,
                        status: WorkloadStatus::Scheduled,
                        assigned_agent_id: Some(agent_id),
                        message: format!("Workload rescheduled to agent {}", agent_id),
                    });
                }
                None => {
                    crate::log_warn!(
                        "scheduler",
                        &format!(
                            "No suitable agent found for reschedule workload_id={}",
                            workload_id
                        )
                    );

                    responses.push(CreateWorkloadResponse {
                        workload_id,
                        status: WorkloadStatus::Pending,
                        assigned_agent_id: None,
                        message: "No agent with sufficient resources available".to_string(),
                    });
                }
            }
        }

        Ok(responses)
    }

    pub async fn retry_pending(&self) -> Result<usize, String> {
        let pending = crate::db::workloads::get_pending(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch pending workloads: {}", e))?;

        if pending.is_empty() {
            return Ok(0);
        }

        let mut agents = crate::db::agents::get_online_agents_with_resources(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        for agent in agents.iter_mut() {
            let (reserved_cpu, reserved_mem, reserved_disk) =
                crate::db::agents::get_assigned_workload_resources(&self.db, agent.agent_id)
                    .await
                    .map_err(|e| format!("Failed to fetch reserved resources: {}", e))?;

            agent.free_cpu_millicores -= reserved_cpu;
            agent.free_memory_bytes -= reserved_mem;
            agent.free_disk_bytes -= reserved_disk;
        }

        let mut placed_count = 0;

        for workload in pending {
            let runtime_class =
                crate::models::workload::RuntimeClass::from_str(&workload.runtime_class);

            let Some(agent_id) = self.first_fit_resources(
                workload.cpu_millicores,
                workload.memory_bytes,
                workload.disk_bytes,
                runtime_class.requires_kvm(),
                &agents,
            ) else {
                crate::log_debug!(
                    "scheduler",
                    &format!(
                        "Pending workload not placeable workload_id={} required_cpu={} required_memory={} required_disk={} requires_kvm={} {}",
                        workload.id,
                        workload.cpu_millicores,
                        workload.memory_bytes,
                        workload.disk_bytes,
                        runtime_class.requires_kvm(),
                        describe_agents(&agents)
                    )
                );
                continue;
            };

            crate::db::workloads::assign(&self.db, workload.id, agent_id)
                .await
                .map_err(|e| format!("Failed to assign workload: {}", e))?;

            let record = PlacementRecord {
                workload_id: workload.id,
                agent_id,
                image: workload.image.clone(),
                cpu_millicores: workload.cpu_millicores,
                memory_bytes: workload.memory_bytes,
                disk_bytes: workload.disk_bytes,
                scheduled_at: Utc::now().to_rfc3339(),
                stack_id: workload.stack_id,
                service_name: workload.service_name.clone(),
                runtime_class: workload.runtime_class.clone(),
            };
            put_placement(&self.etcd, &record).await?;
            tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));

            if let Some(agent) = agents.iter_mut().find(|a| a.agent_id == agent_id) {
                agent.free_cpu_millicores -= workload.cpu_millicores;
                agent.free_memory_bytes -= workload.memory_bytes;
                agent.free_disk_bytes -= workload.disk_bytes;
            }

            crate::log_info!(
                "scheduler",
                &format!(
                    "Pending workload scheduled workload_id={} agent_id={}",
                    workload.id, agent_id
                )
            );

            placed_count += 1;
        }

        Ok(placed_count)
    }

    pub async fn delete_workload(&self, workload_id: Uuid) -> Result<(), String> {
        crate::db::workloads::delete(&self.db, workload_id)
            .await
            .map_err(|e| format!("Failed to delete workload: {}", e))?;

        delete_placement(&self.etcd, workload_id).await?;

        crate::log_info!(
            "scheduler",
            &format!("Workload deleted workload_id={}", workload_id)
        );

        Ok(())
    }
}

fn describe_agents(agents: &[AgentResources]) -> String {
    if agents.is_empty() {
        return "agents=none_online_with_metrics".to_string();
    }
    let entries: Vec<String> = agents
        .iter()
        .map(|agent| {
            format!(
                "agent={}(free_cpu={} free_memory={} free_disk={} kvm={})",
                agent.agent_id,
                agent.free_cpu_millicores,
                agent.free_memory_bytes,
                agent.free_disk_bytes,
                agent.kvm_capable
            )
        })
        .collect();
    format!("agents=[{}]", entries.join(" "))
}

fn format_compose_error(error: &ComposeParseError) -> String {
    match error {
        ComposeParseError::InvalidYaml(detail) => format!("invalid compose yaml: {}", detail),
        ComposeParseError::NoServicesDefined => "compose file defines no services".to_string(),
        ComposeParseError::UnsupportedBuildDirective(service) => {
            format!("service '{}' uses build, only image is supported", service)
        }
        ComposeParseError::MissingImage(service) => {
            format!("service '{}' has no image", service)
        }
    }
}
