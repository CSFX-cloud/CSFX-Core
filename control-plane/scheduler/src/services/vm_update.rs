use entity::entities::workloads;
use shared::vm::VmConfig;
use shared::workload_status::ACTIVE_STATUSES;
use uuid::Uuid;

use crate::models::workload::{Named, RuntimeClass, UpdateWorkloadRequest};
use crate::services::guest_ip;
use crate::services::scheduler::SchedulerService;

const MILLICORES_PER_VCPU: i32 = 1000;
const MIN_MEMORY_BYTES: i64 = 512 * 1024 * 1024;

fn stored_config(workload: &workloads::Model) -> Option<VmConfig> {
    workload
        .vm_config
        .clone()
        .and_then(|config| serde_json::from_value(config).ok())
}

fn own_ips(workload: &workloads::Model) -> Vec<String> {
    let extras = stored_config(workload)
        .map(|config| config.extra_ips().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    workload.guest_ip.iter().cloned().chain(extras).collect()
}

pub fn validate(workload: &workloads::Model, req: &UpdateWorkloadRequest) -> Result<(), String> {
    if !req.touches_vm() {
        return Ok(());
    }
    if workload.runtime_class != RuntimeClass::Vm.name() {
        return Err("only virtual machines can change cpu, memory, disk or network".to_string());
    }
    validate_resources(workload.disk_bytes, req)?;
    let architecture = stored_config(workload).and_then(|stored| stored.architecture);
    validate_config(architecture.as_deref(), req)
}

fn validate_resources(current_disk: i64, req: &UpdateWorkloadRequest) -> Result<(), String> {
    if req
        .cpu_millicores
        .is_some_and(|cpu| cpu < MILLICORES_PER_VCPU || cpu % MILLICORES_PER_VCPU != 0)
    {
        return Err("cpu must be a whole number of vcpus, at least one".to_string());
    }
    if req
        .memory_bytes
        .is_some_and(|memory| memory < MIN_MEMORY_BYTES)
    {
        return Err("memory must be at least 512 MiB".to_string());
    }
    if req.disk_bytes.is_some_and(|disk| disk < current_disk) {
        return Err("the disk can only grow".to_string());
    }
    Ok(())
}

fn validate_config(
    current_architecture: Option<&str>,
    req: &UpdateWorkloadRequest,
) -> Result<(), String> {
    let Some(config) = &req.vm_config else {
        return Ok(());
    };
    config.validate()?;
    if config.architecture.is_some() && config.architecture.as_deref() != current_architecture {
        return Err("the architecture of a vm cannot change".to_string());
    }
    Ok(())
}

fn exceeds(free: i64, current: i64, wanted: Option<i64>) -> bool {
    wanted.is_some_and(|wanted| wanted - current > free)
}

impl SchedulerService {
    pub async fn prepare_vm_update(
        &self,
        workload_id: Uuid,
        mut req: UpdateWorkloadRequest,
    ) -> Result<Option<UpdateWorkloadRequest>, String> {
        let Some(workload) = crate::db::workloads::get_by_id(&self.db, workload_id)
            .await
            .map_err(crate::services::failed("load workload"))?
        else {
            return Ok(None);
        };
        validate(&workload, &req)?;
        self.ensure_capacity(&workload, &req).await?;
        self.resolve_vm_network(&workload, &mut req).await?;
        Ok(Some(req))
    }

    async fn ensure_capacity(
        &self,
        workload: &workloads::Model,
        req: &UpdateWorkloadRequest,
    ) -> Result<(), String> {
        let Some(agent_id) = workload.assigned_agent_id else {
            return Ok(());
        };
        if !req.touches_vm() || !ACTIVE_STATUSES.contains(&workload.status.as_str()) {
            return Ok(());
        }
        let agents = crate::db::agents::get_schedulable_agents(&self.db)
            .await
            .map_err(crate::services::failed("load agents"))?;
        let Some(agent) = agents.iter().find(|agent| agent.agent_id == agent_id) else {
            return Ok(());
        };
        let cpu = req.cpu_millicores.map(i64::from);
        let short = exceeds(
            agent.free_cpu_millicores.into(),
            workload.cpu_millicores.into(),
            cpu,
        ) || exceeds(
            agent.free_memory_bytes,
            workload.memory_bytes,
            req.memory_bytes,
        ) || exceeds(agent.free_disk_bytes, workload.disk_bytes, req.disk_bytes);
        if short {
            return Err("the node does not have enough free resources for this size".to_string());
        }
        Ok(())
    }

    async fn resolve_vm_network(
        &self,
        workload: &workloads::Model,
        req: &mut UpdateWorkloadRequest,
    ) -> Result<(), String> {
        let Some(config) = req.vm_config.clone() else {
            return Ok(());
        };
        let group = workload
            .resource_group_id
            .ok_or_else(|| "ip addresses require a resource group".to_string())?;
        let cidr = crate::db::resource_groups::internal_cidr(&self.db, group)
            .await
            .map_err(crate::services::failed("load resource group cidr"))?
            .ok_or_else(|| format!("Resource group {group} not found"))?;
        let mut used = crate::db::workloads::used_guest_ips(&self.db, group)
            .await
            .map_err(crate::services::failed("load used guest ips"))?;
        for ip in own_ips(workload) {
            used.remove(&ip);
        }
        let ips = guest_ip::assign(&cidr, &used, &config.requested_ips())?;
        req.guest_ip = ips.first().cloned();
        req.vm_config = Some(config.with_assigned_ips(&ips));
        Ok(())
    }
}

#[cfg(test)]
mod tests;
