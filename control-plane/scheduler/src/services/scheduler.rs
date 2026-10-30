use chrono::Utc;
use entity::entities::workloads;
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::models::compose::{ComposeServiceSpec, CreateStackRequest, CreateStackResponse};
use crate::models::workload::{
    AgentResources, CreateWorkloadRequest, CreateWorkloadResponse, RuntimeClass,
    UpdateWorkloadRequest, WorkloadStatus,
};
use crate::services::compose_parser::{self, ComposeParseError};
use crate::services::etcd::{delete_placement, put_placement, PlacementRecord};
use crate::services::placement::{self, Demand, Strategy};

const GUEST_IP_ATTEMPTS: usize = 5;

pub struct SchedulerService {
    db: DatabaseConnection,
    etcd: Arc<Mutex<EtcdClient>>,
    strategy: Strategy,
}

impl SchedulerService {
    pub fn new(db: DatabaseConnection, etcd: Arc<Mutex<EtcdClient>>) -> Self {
        let strategy = Strategy::from_name(
            &std::env::var("SCHEDULER_STRATEGY").unwrap_or_else(|_| "spread".to_string()),
        );
        Self { db, etcd, strategy }
    }

    pub async fn schedule_replicas(
        &self,
        req: CreateWorkloadRequest,
    ) -> Result<Vec<CreateWorkloadResponse>, String> {
        req.validate()?;
        let count = req.replicas.unwrap_or(1);
        if count == 1 {
            return Ok(vec![self.schedule(req).await?]);
        }

        let spread_key = req.spread_key.clone().unwrap_or_else(|| req.name.clone());
        let mut responses = Vec::with_capacity(count as usize);
        for index in 1..=count {
            let replica = CreateWorkloadRequest {
                name: format!("{}-{}", req.name, index),
                spread_key: Some(spread_key.clone()),
                replicas: None,
                ..req.clone()
            };
            responses.push(self.schedule(replica).await?);
        }
        Ok(responses)
    }

    pub async fn schedule(
        &self,
        mut req: CreateWorkloadRequest,
    ) -> Result<CreateWorkloadResponse, String> {
        req.validate()?;
        inject_bucket_env(&mut req).await;

        let workload = self.create_with_guest_ip(&req).await?;

        let mut agents = crate::db::agents::get_schedulable_agents(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        if let Some(required_agent) = self.resolve_volume_affinity(&req).await? {
            agents.retain(|a| a.agent_id == required_agent);
        }

        let preferred = self.hosting_agents(req.resource_group_id).await?;
        let demand = demand_for(&workload, preferred.as_ref());

        let Some(agent_id) = placement::pick(&demand, &agents, self.strategy) else {
            log_unplaceable(&workload, &demand, &agents);
            return Ok(pending_response(workload.id));
        };

        crate::db::workloads::assign(&self.db, workload.id, agent_id)
            .await
            .map_err(|e| format!("Failed to assign workload: {}", e))?;
        self.persist_node_ports(&workload, agent_id, req.ports)
            .await?;
        self.record_placement(&workload, agent_id).await?;

        crate::log_info!(
            "scheduler",
            &format!(
                "Workload scheduled workload_id={} agent_id={}",
                workload.id, agent_id
            )
        );

        Ok(scheduled_response(
            workload.id,
            agent_id,
            "Workload assigned to agent",
        ))
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
        let mut agents = crate::db::agents::get_schedulable_agents(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;
        let mut preferred = self
            .hosting_agents(Some(resource_group_id))
            .await?
            .unwrap_or_default();

        let mut responses = Vec::with_capacity(services.len());
        for service in services {
            let req = stack_service_request(stack_id, resource_group_id, service);
            let workload = self.create_with_guest_ip(&req).await?;

            let demand = demand_for(&workload, Some(&preferred));
            let Some(agent_id) = placement::pick(&demand, &agents, self.strategy) else {
                responses.push(pending_response(workload.id));
                continue;
            };

            self.assign_workload(&workload, agent_id).await?;
            placement::commit(&mut agents, agent_id, &demand);
            preferred.insert(agent_id);
            responses.push(scheduled_response(
                workload.id,
                agent_id,
                "Workload assigned to agent",
            ));
        }

        crate::log_info!(
            "scheduler",
            &format!(
                "Stack scheduled stack_id={} services={}",
                stack_id,
                services.len()
            )
        );

        Ok(responses)
    }

    async fn create_with_guest_ip(
        &self,
        req: &CreateWorkloadRequest,
    ) -> Result<workloads::Model, String> {
        let Some(group) = req.resource_group_id else {
            return crate::db::workloads::create(&self.db, req, None)
                .await
                .map_err(|e| format!("Failed to persist workload: {}", e));
        };

        for _ in 0..GUEST_IP_ATTEMPTS {
            let guest_ip = self.next_guest_ip(group).await?;
            match crate::db::workloads::create(&self.db, req, Some(guest_ip)).await {
                Ok(workload) => return Ok(workload),
                Err(e) if is_unique_violation(&e) => continue,
                Err(e) => return Err(format!("Failed to persist workload: {}", e)),
            }
        }
        Err(format!(
            "Could not reserve a guest ip in resource group {}",
            group
        ))
    }

    async fn next_guest_ip(&self, resource_group_id: Uuid) -> Result<String, String> {
        let cidr = crate::db::resource_groups::internal_cidr(&self.db, resource_group_id)
            .await
            .map_err(|e| format!("Failed to fetch resource group cidr: {}", e))?
            .ok_or_else(|| format!("Resource group {} not found", resource_group_id))?;
        let used = crate::db::workloads::used_guest_ips(&self.db, resource_group_id)
            .await
            .map_err(|e| format!("Failed to fetch used guest ips: {}", e))?;

        crate::services::guest_ip::next_free(&cidr, &used).ok_or_else(|| {
            format!(
                "Resource group {} has no free guest ip in {}",
                resource_group_id, cidr
            )
        })
    }

    pub async fn backfill_guest_ips(&self) -> Result<usize, String> {
        let missing = crate::db::workloads::get_without_guest_ip(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch workloads without guest ip: {}", e))?;

        let mut assigned = 0;
        for workload in missing {
            let Some(group) = workload.resource_group_id else {
                continue;
            };
            let guest_ip = self.next_guest_ip(group).await?;
            crate::db::workloads::set_guest_ip(&self.db, workload.id, guest_ip)
                .await
                .map_err(|e| format!("Failed to store guest ip: {}", e))?;
            assigned += 1;
        }
        Ok(assigned)
    }

    async fn hosting_agents(
        &self,
        resource_group_id: Option<Uuid>,
    ) -> Result<Option<HashSet<Uuid>>, String> {
        let Some(resource_group_id) = resource_group_id else {
            return Ok(None);
        };
        crate::db::agents::get_agents_hosting_resource_group(&self.db, resource_group_id)
            .await
            .map(Some)
            .map_err(|e| format!("Failed to resolve resource group affinity: {}", e))
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

    async fn persist_node_ports(
        &self,
        workload: &workloads::Model,
        agent_id: Uuid,
        ports: Option<Vec<crate::models::workload::PortMapping>>,
    ) -> Result<(), String> {
        let Some(mut ports) = ports else {
            return Ok(());
        };
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
            .map_err(|e| format!("Failed to persist allocated node ports: {}", e))
    }

    async fn assign_workload(
        &self,
        workload: &workloads::Model,
        agent_id: Uuid,
    ) -> Result<(), String> {
        crate::db::workloads::assign(&self.db, workload.id, agent_id)
            .await
            .map_err(|e| format!("Failed to assign workload: {}", e))?;
        self.record_placement(workload, agent_id).await
    }

    async fn record_placement(
        &self,
        workload: &workloads::Model,
        agent_id: Uuid,
    ) -> Result<(), String> {
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
        Ok(())
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
        let mut agents = crate::db::agents::get_schedulable_agents(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        agents.retain(|a| a.agent_id != dead_agent_id);

        let mut responses = Vec::with_capacity(workload_ids.len());

        for &workload_id in workload_ids {
            let workload = crate::db::workloads::get_by_id(&self.db, workload_id)
                .await
                .map_err(|e| format!("Failed to fetch workload: {}", e))?
                .ok_or_else(|| format!("Workload {} not found", workload_id))?;

            let preferred = self.hosting_agents(workload.resource_group_id).await?;
            let demand = demand_for(&workload, preferred.as_ref());
            let Some(agent_id) = placement::pick(&demand, &agents, self.strategy) else {
                crate::log_warn!(
                    "scheduler",
                    &format!(
                        "No suitable agent found for reschedule workload_id={}",
                        workload_id
                    )
                );
                responses.push(pending_response(workload_id));
                continue;
            };

            self.assign_workload(&workload, agent_id).await?;
            placement::commit(&mut agents, agent_id, &demand);

            crate::log_info!(
                "scheduler",
                &format!(
                    "Workload rescheduled workload_id={} from_agent_id={} to_agent_id={}",
                    workload_id, dead_agent_id, agent_id
                )
            );

            responses.push(scheduled_response(
                workload_id,
                agent_id,
                "Workload rescheduled to agent",
            ));
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

        let mut agents = crate::db::agents::get_schedulable_agents(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))?;

        let mut placed_count = 0;

        for workload in pending {
            let preferred = self.hosting_agents(workload.resource_group_id).await?;
            let demand = demand_for(&workload, preferred.as_ref());
            let Some(agent_id) = placement::pick(&demand, &agents, self.strategy) else {
                crate::log_debug!(
                    "scheduler",
                    &format!(
                        "Pending workload not placeable workload_id={} required_cpu={} required_memory={} required_disk={} requires_kvm={} {}",
                        workload.id,
                        demand.cpu_millicores,
                        demand.memory_bytes,
                        demand.disk_bytes,
                        demand.requires_kvm,
                        describe_agents(&agents)
                    )
                );
                continue;
            };

            self.assign_workload(&workload, agent_id).await?;
            placement::commit(&mut agents, agent_id, &demand);

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

fn is_unique_violation(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

async fn inject_bucket_env(req: &mut CreateWorkloadRequest) {
    let Some(bindings) = req.bucket_bindings.clone().filter(|b| !b.is_empty()) else {
        return;
    };
    let bucket_env = crate::services::bucket_bindings::resolve_env_vars(
        Uuid::new_v4(),
        req.resource_group_id,
        &bindings,
    )
    .await;
    req.env_vars
        .get_or_insert_with(std::collections::HashMap::new)
        .extend(bucket_env);
}

fn demand_for<'a>(
    workload: &'a workloads::Model,
    preferred_agents: Option<&'a HashSet<Uuid>>,
) -> Demand<'a> {
    Demand {
        cpu_millicores: workload.cpu_millicores,
        memory_bytes: workload.memory_bytes,
        disk_bytes: workload.disk_bytes,
        requires_kvm: RuntimeClass::from_str(&workload.runtime_class).requires_kvm(),
        spread_key: workload.spread_key.as_deref(),
        preferred_agents,
    }
}

fn stack_service_request(
    stack_id: Uuid,
    resource_group_id: Uuid,
    service: &ComposeServiceSpec,
) -> CreateWorkloadRequest {
    CreateWorkloadRequest {
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
        runtime_class: RuntimeClass::Firecracker,
        cpu_limit_millicores: service.cpu_limit_millicores,
        memory_limit_bytes: service.memory_limit_bytes,
        auto_resources: false,
        replicas: None,
        spread_key: Some(stack_id.to_string()),
    }
}

fn scheduled_response(workload_id: Uuid, agent_id: Uuid, message: &str) -> CreateWorkloadResponse {
    CreateWorkloadResponse {
        workload_id,
        status: WorkloadStatus::Scheduled,
        assigned_agent_id: Some(agent_id),
        message: format!("{} {}", message, agent_id),
    }
}

fn pending_response(workload_id: Uuid) -> CreateWorkloadResponse {
    CreateWorkloadResponse {
        workload_id,
        status: WorkloadStatus::Pending,
        assigned_agent_id: None,
        message: "No agent with sufficient resources available".to_string(),
    }
}

fn log_unplaceable(workload: &workloads::Model, demand: &Demand, agents: &[AgentResources]) {
    crate::log_warn!(
        "scheduler",
        &format!(
            "No suitable agent found workload_id={} required_cpu={} required_memory={} required_disk={} requires_kvm={} {}",
            workload.id,
            demand.cpu_millicores,
            demand.memory_bytes,
            demand.disk_bytes,
            demand.requires_kvm,
            describe_agents(agents)
        )
    );
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
        ComposeParseError::InvalidResources(service) => {
            format!("service '{}' has invalid deploy resources", service)
        }
    }
}
