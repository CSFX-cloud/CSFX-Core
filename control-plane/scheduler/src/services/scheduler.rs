use entity::entities::workloads;
use sea_orm::{ConnectionTrait, DatabaseConnection, DatabaseTransaction, TransactionTrait};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::models::workload::{
    node_ports_of, AgentResources, CreateWorkloadRequest, CreateWorkloadResponse, Named,
    TopologySpread, VolumeMount, WorkloadStatus,
};
use crate::services::image_resolver;
use crate::services::placement::{self, Demand, Strategy};

pub(crate) fn notify_assigned(agent_id: Uuid) {
    tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
}

pub(crate) fn notify_assignee(workload: &workloads::Model) {
    if let Some(agent_id) = workload.assigned_agent_id {
        notify_assigned(agent_id);
    }
}

pub(crate) const REPLACEMENT_EXISTS: &str = "an update of this workload is already in progress";
pub const STACK_BUSY: &str = "a rollout is in progress for this stack";
pub(crate) const NO_SURGE_TARGET: &str =
    "no agent has room for a replacement next to the running workload";

pub struct SchedulerService {
    pub(crate) db: DatabaseConnection,
    strategy: Strategy,
    pub(crate) ready_since:
        std::sync::Mutex<HashMap<Uuid, (std::time::Instant, Option<chrono::NaiveDateTime>)>>,
}

impl SchedulerService {
    pub fn new(db: DatabaseConnection) -> Self {
        let strategy = Strategy::from_name(
            &std::env::var("SCHEDULER_STRATEGY").unwrap_or_else(|_| "spread".to_string()),
        );
        Self {
            db,
            strategy,
            ready_since: std::sync::Mutex::new(HashMap::new()),
        }
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
        let mut responses = Vec::new();
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
        req: CreateWorkloadRequest,
    ) -> Result<CreateWorkloadResponse, String> {
        self.schedule_with(req, None).await
    }

    pub(crate) async fn schedule_with(
        &self,
        mut req: CreateWorkloadRequest,
        replacement: Option<Uuid>,
    ) -> Result<CreateWorkloadResponse, String> {
        req.validate()?;
        inject_bucket_env(&mut req).await;
        req.image = image_resolver::resolve(req.resource_group_id, &req.image).await;
        req.replaces = replacement;

        let workload = self.create_with_guest_ip(&req).await?;
        let workload_id = workload.id;
        let placed = self.place(workload, req, replacement).await;
        if placed.is_err() && replacement.is_some() {
            if let Err(error) = crate::db::workloads::delete(&self.db, workload_id).await {
                tracing::warn!(workload = %workload_id, error = %error, "replacement cleanup failed");
            }
        }
        placed
    }

    async fn place(
        &self,
        workload: workloads::Model,
        req: CreateWorkloadRequest,
        replaced: Option<Uuid>,
    ) -> Result<CreateWorkloadResponse, String> {
        let mut agents = self.schedulable_agents().await?;

        if let Some(required_agent) = self.resolve_volume_affinity(&req).await? {
            agents.retain(|a| a.agent_id == required_agent);
        }

        let preferred = self.hosting_agents(req.resource_group_id).await?;
        let demand = demand_for(&workload, preferred.as_ref());

        let Some(agent_id) = placement::pick(&demand, &agents, self.strategy) else {
            if replaced.is_some() {
                return Err(NO_SURGE_TARGET.to_string());
            }
            crate::log_warn!(
                "scheduler",
                &unplaceable_message(&workload, &demand, &agents)
            );
            return Ok(pending_response(workload.id));
        };

        if !self.assign_workload(&workload, agent_id, None).await? {
            return Ok(pending_response(workload.id));
        }

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

    pub(crate) async fn schedulable_agents(&self) -> Result<Vec<AgentResources>, String> {
        crate::db::agents::get_schedulable_agents(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch agent resources: {}", e))
    }

    pub(crate) async fn place_batch(
        &self,
        workloads: &[workloads::Model],
        agents: &mut [AgentResources],
        expected_agent: Option<Uuid>,
    ) -> Result<Vec<Option<Uuid>>, String> {
        let mut placements = Vec::with_capacity(workloads.len());
        for workload in workloads {
            let placed = self.place_next(workload, agents, expected_agent).await?;
            if placed.is_none() {
                if let Some(agent_id) = expected_agent {
                    self.release_to_pending(workload.id, agent_id).await?;
                    crate::log_warn!(
                        "scheduler",
                        &format!(
                            "Workload released to pending workload_id={} from_agent_id={}",
                            workload.id, agent_id
                        )
                    );
                }
            }
            placements.push(placed);
        }
        Ok(placements)
    }

    async fn place_next(
        &self,
        workload: &workloads::Model,
        agents: &mut [AgentResources],
        expected_agent: Option<Uuid>,
    ) -> Result<Option<Uuid>, String> {
        let preferred = self.hosting_agents(workload.resource_group_id).await?;
        let demand = demand_for(workload, preferred.as_ref());
        let Some(agent_id) = placement::pick(&demand, agents, self.strategy) else {
            crate::log_debug!("scheduler", &unplaceable_message(workload, &demand, agents));
            return Ok(None);
        };
        if !self
            .assign_workload(workload, agent_id, expected_agent)
            .await?
        {
            return Ok(None);
        }
        placement::commit(agents, agent_id, &demand);
        crate::log_info!(
            "scheduler",
            &format!(
                "Workload placed workload_id={} agent_id={}",
                workload.id, agent_id
            )
        );
        Ok(Some(agent_id))
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

    async fn assign_workload(
        &self,
        workload: &workloads::Model,
        agent_id: Uuid,
        expected_agent: Option<Uuid>,
    ) -> Result<bool, String> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|e| format!("Failed to open assignment transaction: {}", e))?;
        let assigned =
            crate::db::workloads::assign(&transaction, workload.id, agent_id, expected_agent)
                .await
                .map_err(|e| format!("Failed to assign workload: {}", e))?;
        if !assigned {
            crate::log_warn!(
                "scheduler",
                &format!(
                    "Workload already placed by another scheduler workload_id={} agent_id={}",
                    workload.id, agent_id
                )
            );
            return rolled_back(transaction).await;
        }
        if !claim_volumes(&transaction, workload, agent_id).await? {
            return rolled_back(transaction).await;
        }
        transaction
            .commit()
            .await
            .map_err(|e| format!("Failed to commit assignment: {}", e))?;
        notify_assigned(agent_id);
        Ok(true)
    }

    pub async fn list_workloads(
        &self,
        after: Option<Uuid>,
        params: &shared::pagination::PageParams,
    ) -> Result<Vec<crate::models::workload::WorkloadResponse>, String> {
        crate::db::workloads::get_page(&self.db, after, params)
            .await
            .map_err(|e| format!("Failed to list workloads: {}", e))
    }

    pub async fn reschedule_from_agent(
        &self,
        dead_agent_id: Uuid,
        workload_ids: &[Uuid],
    ) -> Result<Vec<CreateWorkloadResponse>, String> {
        let mut agents = self.schedulable_agents().await?;
        agents.retain(|a| a.agent_id != dead_agent_id);

        let mut stranded = Vec::with_capacity(workload_ids.len());
        for &workload_id in workload_ids {
            let workload = crate::db::workloads::get_by_id(&self.db, workload_id)
                .await
                .map_err(|e| format!("Failed to fetch workload: {}", e))?
                .ok_or_else(|| format!("Workload {} not found", workload_id))?;
            stranded.push(workload);
        }

        let placements = self
            .place_batch(&stranded, &mut agents, Some(dead_agent_id))
            .await?;

        Ok(stranded
            .iter()
            .zip(placements)
            .map(|(workload, placed)| {
                response_for(workload.id, placed, "Workload rescheduled to agent")
            })
            .collect())
    }

    async fn release_to_pending(&self, workload_id: Uuid, agent_id: Uuid) -> Result<(), String> {
        crate::db::workloads::unassign(&self.db, workload_id, agent_id)
            .await
            .map_err(|e| format!("Failed to release workload to pending: {}", e))?;
        Ok(())
    }

    pub async fn retry_pending(&self) -> Result<usize, String> {
        let pending = crate::db::workloads::get_pending(&self.db)
            .await
            .map_err(|e| format!("Failed to fetch pending workloads: {}", e))?;

        if pending.is_empty() {
            return Ok(0);
        }

        let mut agents = self.schedulable_agents().await?;
        let placements = self.place_batch(&pending, &mut agents, None).await?;

        Ok(placements.iter().flatten().count())
    }

    pub async fn delete_workload(&self, workload_id: Uuid) -> Result<(), String> {
        crate::db::workloads::delete(&self.db, workload_id)
            .await
            .map_err(|e| format!("Failed to delete workload: {}", e))?;

        shared::volumes::release_for_workload(&self.db, workload_id)
            .await
            .map_err(|e| format!("Failed to release workload volumes: {}", e))?;

        crate::log_info!(
            "scheduler",
            &format!("Workload deleted workload_id={}", workload_id)
        );

        Ok(())
    }
}

async fn rolled_back(transaction: DatabaseTransaction) -> Result<bool, String> {
    transaction
        .rollback()
        .await
        .map_err(|e| format!("Failed to roll back assignment: {}", e))?;
    Ok(false)
}

async fn claim_volumes(
    db: &impl ConnectionTrait,
    workload: &workloads::Model,
    agent_id: Uuid,
) -> Result<bool, String> {
    let volume_ids = volume_ids_of(workload);
    let claimed = shared::volumes::claim_all(db, &volume_ids, agent_id, Some(workload.id))
        .await
        .map_err(|e| format!("Failed to claim volumes: {}", e))?;

    if !claimed {
        crate::log_warn!(
            "scheduler",
            &format!(
                "Volumes held by another workload or agent workload_id={} agent_id={}",
                workload.id, agent_id
            )
        );
    }

    Ok(claimed)
}

fn volume_ids_of(workload: &workloads::Model) -> Vec<Uuid> {
    workload
        .volume_mounts
        .as_ref()
        .and_then(|mounts| serde_json::from_value::<Vec<VolumeMount>>(mounts.clone()).ok())
        .unwrap_or_default()
        .into_iter()
        .map(|mount| mount.volume_id)
        .collect()
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
        spread_key: workload.spread_key.as_deref(),
        topology_spread: TopologySpread::from_name(&workload.topology_spread),
        preferred_agents,
        workload_id: workload.id,
        replaces: workload.replaces_workload_id,
        node_ports: workload
            .ports
            .as_ref()
            .map(node_ports_of)
            .unwrap_or_default(),
        architecture: workload
            .vm_config
            .as_ref()
            .and_then(|config| config["architecture"].as_str()),
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

pub(crate) fn response_for(
    workload_id: Uuid,
    placed: Option<Uuid>,
    message: &str,
) -> CreateWorkloadResponse {
    match placed {
        Some(agent_id) => scheduled_response(workload_id, agent_id, message),
        None => pending_response(workload_id),
    }
}

fn unplaceable_message(
    workload: &workloads::Model,
    demand: &Demand,
    agents: &[AgentResources],
) -> String {
    format!(
        "No suitable agent found workload_id={} required_cpu={} required_memory={} required_disk={} {}",
        workload.id,
        demand.cpu_millicores,
        demand.memory_bytes,
        demand.disk_bytes,
        describe_agents(agents)
    )
}

fn describe_agents(agents: &[AgentResources]) -> String {
    if agents.is_empty() {
        return "agents=none_online_with_metrics".to_string();
    }
    let entries: Vec<String> = agents
        .iter()
        .map(|agent| {
            format!(
                "agent={}(free_cpu={} free_memory={} free_disk={} kvm={} arch={})",
                agent.agent_id,
                agent.free_cpu_millicores,
                agent.free_memory_bytes,
                agent.free_disk_bytes,
                agent.kvm_capable,
                agent.architecture
            )
        })
        .collect();
    format!("agents=[{}]", entries.join(" "))
}
