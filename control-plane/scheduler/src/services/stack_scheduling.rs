use entity::entities::workloads;
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::compose::{ComposeServiceSpec, CreateStackRequest, CreateStackResponse};
use crate::models::workload::{
    CreateWorkloadRequest, CreateWorkloadResponse, RuntimeClass, TopologySpread,
    UpdateWorkloadRequest, VolumeMount,
};
use crate::services::compose_parser::{self, ComposeParseError};
use crate::services::scheduler::{notify_assignee, response_for, SchedulerService, STACK_BUSY};
use crate::services::{image_resolver, stack_volumes};

impl SchedulerService {
    pub async fn schedule_stack(
        &self,
        req: CreateStackRequest,
    ) -> Result<CreateStackResponse, String> {
        let mut services = compose_parser::parse_compose(&req.compose_yaml)
            .map_err(|e| format_compose_error(&e))?;
        resolve_service_images(req.resource_group_id, &mut services).await;
        let volume_ids =
            stack_volumes::ensure(&self.db, req.resource_group_id, &req.name, &services).await?;

        let stack = crate::db::workload_stacks::create(
            &self.db,
            req.resource_group_id,
            &req.name,
            &req.compose_yaml,
        )
        .await
        .map_err(|e| format!("Failed to persist stack: {}", e))?;

        let workloads = self
            .place_stack_workloads(stack.id, req.resource_group_id, &services, &volume_ids)
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

    async fn stack_workloads(&self, stack_id: Uuid) -> Result<Vec<workloads::Model>, String> {
        crate::db::workloads::get_by_stack_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to fetch stack workloads: {}", e))
    }

    pub async fn delete_stack(&self, stack_id: Uuid) -> Result<(), String> {
        let workloads = self.stack_workloads(stack_id).await?;

        for workload in &workloads {
            shared::volumes::release_for_workload(&self.db, workload.id)
                .await
                .map_err(|e| format!("Failed to release workload volumes: {}", e))?;
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
        for workload in self.stack_workloads(stack_id).await? {
            let model = crate::db::workloads::set_desired_state(
                &self.db,
                workload.id,
                crate::models::workload::DesiredState::Stopped,
            )
            .await
            .map_err(|e| format!("Failed to stop workload: {}", e))?;
            notify_assignee(&model);
        }

        Ok(())
    }

    pub async fn restart_stack(&self, stack_id: Uuid) -> Result<(), String> {
        let workloads = self.stack_workloads(stack_id).await?;
        ensure_stack_idle(&workloads)?;

        for workload in &workloads {
            let model = self
                .apply_restart(workload.id)
                .await
                .map_err(|e| format!("Failed to restart workload: {}", e))?;
            notify_assignee(&model);
        }

        Ok(())
    }

    pub async fn redeploy_stack(&self, stack_id: Uuid, compose_yaml: &str) -> Result<(), String> {
        let mut services =
            compose_parser::parse_compose(compose_yaml).map_err(|e| format_compose_error(&e))?;
        let stack = crate::db::workload_stacks::get_by_id(&self.db, stack_id)
            .await
            .map_err(|e| format!("Failed to fetch stack: {}", e))?
            .ok_or_else(|| format!("Stack {} not found", stack_id))?;
        resolve_service_images(stack.resource_group_id, &mut services).await;

        let existing = self.stack_workloads(stack_id).await?;
        ensure_stack_idle(&existing)?;

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
                volume_mounts: None,
            };

            let model = self
                .apply_update(workload.id, &update)
                .await
                .map_err(|e| format!("Failed to update workload: {}", e))?;
            notify_assignee(&model);
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
        volume_ids: &HashMap<String, Uuid>,
    ) -> Result<Vec<CreateWorkloadResponse>, String> {
        let mut created = Vec::with_capacity(services.len());
        for service in services {
            let req = stack_service_request(stack_id, resource_group_id, service, volume_ids);
            created.push(self.create_with_guest_ip(&req).await?);
        }

        let mut agents = self.schedulable_agents().await?;
        let placements = self.place_batch(&created, &mut agents, None).await?;

        crate::log_info!(
            "scheduler",
            &format!(
                "Stack scheduled stack_id={} services={}",
                stack_id,
                services.len()
            )
        );

        Ok(created
            .iter()
            .zip(placements)
            .map(|(workload, placed)| {
                response_for(workload.id, placed, "Workload assigned to agent")
            })
            .collect())
    }
}

fn ensure_stack_idle(workloads: &[workloads::Model]) -> Result<(), String> {
    let busy = workloads
        .iter()
        .any(|workload| workload.replaces_workload_id.is_some() || workload.retire_after.is_some());
    if busy {
        return Err(STACK_BUSY.to_string());
    }
    Ok(())
}

async fn resolve_service_images(resource_group_id: Uuid, services: &mut [ComposeServiceSpec]) {
    for service in services {
        service.image = image_resolver::resolve(Some(resource_group_id), &service.image).await;
    }
}

fn stack_volume_mounts(
    service: &ComposeServiceSpec,
    volume_ids: &HashMap<String, Uuid>,
) -> Option<Vec<VolumeMount>> {
    let mounts: Vec<VolumeMount> = service
        .volumes
        .iter()
        .filter_map(|mount| {
            Some(VolumeMount {
                volume_id: *volume_ids.get(&mount.source)?,
                mount_path: mount.mount_path.clone(),
            })
        })
        .collect();
    (!mounts.is_empty()).then_some(mounts)
}

fn stack_service_request(
    stack_id: Uuid,
    resource_group_id: Uuid,
    service: &ComposeServiceSpec,
    volume_ids: &HashMap<String, Uuid>,
) -> CreateWorkloadRequest {
    CreateWorkloadRequest {
        name: format!("{}-{}", stack_id, service.service_name),
        image: service.image.clone(),
        cpu_millicores: service.cpu_millicores,
        memory_bytes: service.memory_bytes,
        disk_bytes: service.disk_bytes,
        env_vars: service.env_vars.clone(),
        ports: service.ports.clone(),
        volume_mounts: stack_volume_mounts(service, volume_ids),
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
        topology_spread: TopologySpread::None,
        vm_config: None,
        replaces: None,
    }
}

fn format_compose_error(error: &ComposeParseError) -> String {
    error.to_string()
}
