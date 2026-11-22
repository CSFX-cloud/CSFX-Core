use super::failed;
use chrono::Utc;
use entity::entities::{
    agents, dns_records, ingress_gateways, ingress_routes, logs, network_members, site_deployments,
    workloads,
};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter, QuerySelect,
    TransactionTrait,
};
use shared::env_secrets;
use shared::rollout::{abandoned_attempts, ABANDONED_KEY};
use shared::vm::VmConfig;
use std::time::{Duration, Instant};
use uuid::Uuid;

use crate::models::workload::{
    CreateWorkloadRequest, DesiredState, Named, PortMapping, RestartPolicy, RuntimeClass,
    TopologySpread, UpdateWorkloadRequest, WorkloadStatus,
};
use crate::services::rightsizing::{recommend_cpu_millicores, recommend_memory_bytes};
use crate::services::scheduler::{
    notify_assigned, SchedulerService, NO_SURGE_TARGET, REPLACEMENT_EXISTS,
};

const READY_HOLD: Duration = Duration::from_secs(10);
const MAX_REPLACEMENT_AGE_SECONDS: i64 = 600;
const RETIRE_GRACE_SECONDS: i64 = 75;
const TOGGLE_VARIABLE: &str = "CSFX_SURGE_UPDATES";
const VM_CLASS: &str = "vm";

pub fn surge_enabled(value: Option<&str>) -> bool {
    !matches!(
        value.map(|raw| raw.trim().to_ascii_lowercase()).as_deref(),
        Some("false" | "0" | "no" | "off")
    )
}

pub fn touches_spec(update: &UpdateWorkloadRequest) -> bool {
    update.image.is_some() || update.env_vars.is_some() || update.ports.is_some()
}

#[derive(Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    Switch,
    Abandon,
}

pub fn next_step(
    replaced_exists: bool,
    replaced_desired_running: bool,
    status: WorkloadStatus,
    age_seconds: i64,
    ready_for: Option<Duration>,
) -> Step {
    if !replaced_exists || !replaced_desired_running {
        return Step::Abandon;
    }
    if status == WorkloadStatus::Failed || age_seconds > MAX_REPLACEMENT_AGE_SECONDS {
        return Step::Abandon;
    }
    match ready_for {
        Some(held) if status == WorkloadStatus::Running && held >= READY_HOLD => Step::Switch,
        _ => Step::Wait,
    }
}

fn opened_env(workload: &workloads::Model) -> Option<std::collections::HashMap<String, String>> {
    let stored = workload.env_vars.as_ref()?;
    env_secrets::open_map(stored, &env_secrets::workload_env(workload.id)).ok()
}

fn same_spec(old: &workloads::Model, replacement: &workloads::Model) -> bool {
    old.image == replacement.image
        && old.ports == replacement.ports
        && opened_env(old) == opened_env(replacement)
}

fn abandon_reason(failed: bool, age_seconds: i64) -> &'static str {
    if failed {
        "the new instance failed to start"
    } else if age_seconds > MAX_REPLACEMENT_AGE_SECONDS {
        "the new instance did not become ready in time"
    } else {
        "the workload was stopped or deleted"
    }
}

struct Rightsized {
    cpu_millicores: i32,
    cpu_limit_millicores: Option<i32>,
    memory_bytes: i64,
    memory_limit_bytes: Option<i64>,
}

fn rightsized(workload: &workloads::Model) -> Rightsized {
    let mut sizes = Rightsized {
        cpu_millicores: workload.cpu_millicores,
        cpu_limit_millicores: workload.cpu_limit_millicores,
        memory_bytes: workload.memory_bytes,
        memory_limit_bytes: workload.memory_limit_bytes,
    };
    if !workload.auto_resources {
        return sizes;
    }
    if let Some(peak) = workload.cpu_usage_peak_percent {
        let ceiling = workload
            .cpu_limit_millicores
            .unwrap_or(workload.cpu_millicores);
        sizes.cpu_limit_millicores = Some(ceiling);
        sizes.cpu_millicores = recommend_cpu_millicores(peak).min(ceiling);
    }
    if let Some(peak) = workload.memory_usage_peak_bytes {
        let ceiling = workload.memory_limit_bytes.unwrap_or(workload.memory_bytes);
        sizes.memory_limit_bytes = Some(ceiling);
        sizes.memory_bytes = recommend_memory_bytes(peak).min(ceiling);
    }
    sizes
}

fn has_volumes(workload: &workloads::Model) -> bool {
    workload
        .volume_mounts
        .as_ref()
        .and_then(|mounts| mounts.as_array())
        .is_some_and(|mounts| !mounts.is_empty())
}

fn typed_ports(workload: &workloads::Model) -> Option<Vec<PortMapping>> {
    workload
        .ports
        .as_ref()
        .and_then(|ports| serde_json::from_value(ports.clone()).ok())
}

fn in_rollout(workload: &workloads::Model) -> bool {
    workload.replaces_workload_id.is_some() || workload.retire_after.is_some()
}

impl SchedulerService {
    async fn ensure_not_in_rollout(&self, workload: &workloads::Model) -> Result<(), DbErr> {
        let replaced = crate::db::workloads::replacement_for(&self.db, workload.id).await?;
        if in_rollout(workload) || replaced.is_some() {
            return Err(DbErr::RecordNotUpdated);
        }
        Ok(())
    }

    async fn set_rollout_error(
        &self,
        workload: &workloads::Model,
        message: Option<&str>,
    ) -> Result<(), DbErr> {
        if workload.rollout_error.is_none() && message.is_none() {
            return Ok(());
        }
        workloads::Entity::update_many()
            .col_expr(
                workloads::Column::RolloutError,
                Expr::value(message.map(str::to_string)),
            )
            .filter(workloads::Column::Id.eq(workload.id))
            .exec(&self.db)
            .await?;
        Ok(())
    }

    pub async fn apply_update(
        &self,
        workload_id: Uuid,
        update: &UpdateWorkloadRequest,
    ) -> Result<workloads::Model, DbErr> {
        let old = crate::db::workloads::get_by_id(&self.db, workload_id)
            .await?
            .ok_or(DbErr::RecordNotFound(workload_id.to_string()))?;
        self.ensure_not_in_rollout(&old).await?;
        self.set_rollout_error(&old, None).await?;
        if touches_spec(update) && self.try_replace(&old, update).await? {
            return Ok(old);
        }
        crate::db::workloads::update_spec(&self.db, workload_id, update).await
    }

    pub async fn apply_restart(&self, workload_id: Uuid) -> Result<workloads::Model, DbErr> {
        let old = crate::db::workloads::get_by_id(&self.db, workload_id)
            .await?
            .ok_or(DbErr::RecordNotFound(workload_id.to_string()))?;
        self.ensure_not_in_rollout(&old).await?;
        self.set_rollout_error(&old, None).await?;
        let unchanged = UpdateWorkloadRequest {
            image: None,
            env_vars: None,
            ports: None,
            restart_policy: None,
            max_restarts: None,
            volume_mounts: None,
        };
        if self.try_replace(&old, &unchanged).await? {
            return Ok(old);
        }
        crate::db::workloads::request_restart(&self.db, workload_id).await
    }

    async fn try_replace(
        &self,
        old: &workloads::Model,
        update: &UpdateWorkloadRequest,
    ) -> Result<bool, DbErr> {
        if !surge_enabled(std::env::var(TOGGLE_VARIABLE).ok().as_deref()) {
            return Ok(false);
        }
        match self.eligible(old, update).await {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(error) => {
                tracing::warn!(workload = %old.id, error = %error, "surge eligibility check failed");
                return Ok(false);
            }
        }
        match self.start_replacement(old, update).await {
            Ok(()) => Ok(true),
            Err(message) if message == REPLACEMENT_EXISTS => Err(DbErr::RecordNotUpdated),
            Err(message) if message == NO_SURGE_TARGET => Ok(false),
            Err(message) => {
                tracing::warn!(workload = %old.id, error = %message, "surge update failed, updating in place");
                Ok(false)
            }
        }
    }

    async fn eligible(
        &self,
        old: &workloads::Model,
        update: &UpdateWorkloadRequest,
    ) -> Result<bool, DbErr> {
        let runnable = old.assigned_agent_id.is_some()
            && old.status == WorkloadStatus::Running.name()
            && old.desired_state == DesiredState::Running.name()
            && !old.restart_requested
            && old.runtime_class != VM_CLASS;
        let stateless = !has_volumes(old)
            && update
                .volume_mounts
                .as_ref()
                .is_none_or(|mounts| mounts.is_empty());
        if !runnable || !stateless {
            return Ok(false);
        }
        Ok(!self.system_managed(old.id).await?)
    }

    async fn system_managed(&self, workload_id: Uuid) -> Result<bool, DbErr> {
        let gateways = ingress_gateways::Entity::find()
            .filter(ingress_gateways::Column::WorkloadId.eq(workload_id))
            .count(&self.db)
            .await?;
        let deployments = site_deployments::Entity::find()
            .filter(site_deployments::Column::WorkloadId.eq(workload_id))
            .count(&self.db)
            .await?;
        Ok(gateways + deployments > 0)
    }

    async fn start_replacement(
        &self,
        old: &workloads::Model,
        update: &UpdateWorkloadRequest,
    ) -> Result<(), String> {
        let request = replacement_request(old, update)?;
        let created = self.schedule_with(request, Some(old.id)).await?;
        if update.env_vars.is_none() {
            self.carry_versions(created.workload_id, old).await;
        }
        tracing::info!(workload = %old.id, "replacement started next to the running workload");
        Ok(())
    }

    async fn carry_versions(&self, replacement_id: Uuid, old: &workloads::Model) {
        let carried = old
            .secret_versions
            .clone()
            .filter(|recorded| recorded.get(ABANDONED_KEY).is_none());
        let result = workloads::Entity::update_many()
            .col_expr(workloads::Column::SecretVersions, Expr::value(carried))
            .filter(workloads::Column::Id.eq(replacement_id))
            .exec(&self.db)
            .await;
        if let Err(error) = result {
            tracing::warn!(workload = %replacement_id, error = %error, "secret versions were not carried over");
        }
    }

    pub async fn reconcile_replacements(&self) -> Result<usize, String> {
        let pending = crate::db::workloads::replacements(&self.db)
            .await
            .map_err(failed("list replacements"))?;
        let mut switched = 0;
        for replacement in pending {
            match self.advance(&replacement).await {
                Ok(true) => switched += 1,
                Ok(false) => {}
                Err(error) => {
                    tracing::warn!(workload = %replacement.id, error = %error, "replacement step failed")
                }
            }
        }
        self.retire_due().await?;
        Ok(switched)
    }

    async fn retire_due(&self) -> Result<(), String> {
        let due = crate::db::workloads::retiring_due(&self.db, Utc::now().naive_utc())
            .await
            .map_err(failed("list retiring workloads"))?;
        for old in due {
            match self.delete_workload(old.id).await {
                Ok(()) => tracing::info!(workload = %old.id, "previous instance retired"),
                Err(error) => {
                    tracing::warn!(workload = %old.id, error = %error, "retiring the previous instance failed")
                }
            }
            if let Some(agent_id) = old.assigned_agent_id {
                tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
            }
        }
        Ok(())
    }

    async fn advance(&self, replacement: &workloads::Model) -> Result<bool, String> {
        let Some(replaced_id) = replacement.replaces_workload_id else {
            return Ok(false);
        };
        let replaced = crate::db::workloads::get_by_id(&self.db, replaced_id)
            .await
            .map_err(|e| e.to_string())?;
        let status = WorkloadStatus::from_name(&replacement.status);
        let running = status == WorkloadStatus::Running;
        let failed = status == WorkloadStatus::Failed;
        let age = (Utc::now().naive_utc() - replacement.created_at).num_seconds();
        let step = next_step(
            replaced.is_some(),
            replaced
                .as_ref()
                .is_some_and(|workload| workload.desired_state == DesiredState::Running.name()),
            status,
            age,
            self.ready_for(replacement.id, running, replacement.updated_at),
        );
        match step {
            Step::Switch => self.switch_over(replaced_id, replacement.id).await,
            Step::Abandon => self
                .abandon(replacement, replaced.as_ref(), abandon_reason(failed, age))
                .await
                .map(|_| false),
            Step::Wait => Ok(false),
        }
    }

    async fn abandon(
        &self,
        replacement: &workloads::Model,
        replaced: Option<&workloads::Model>,
        reason: &str,
    ) -> Result<(), String> {
        tracing::warn!(workload = %replacement.id, reason, "replacement abandoned, keeping the previous workload");
        self.forget_ready(replacement.id);
        if let Some(old) = replaced {
            let message = format!("The update was rolled back: {reason}");
            self.set_rollout_error(old, Some(&message))
                .await
                .map_err(|e| e.to_string())?;
        }
        if let Some(old) = replaced.filter(|old| old.secret_versions.is_some()) {
            if same_spec(old, replacement) {
                self.mark_rotation_pending(old).await?;
            }
        }
        self.delete_workload(replacement.id).await?;
        if let Some(agent_id) = replacement.assigned_agent_id {
            tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
        }
        Ok(())
    }

    async fn mark_rotation_pending(&self, old: &workloads::Model) -> Result<(), String> {
        let attempts = abandoned_attempts(old.secret_versions.as_ref()) + 1;
        workloads::Entity::update_many()
            .col_expr(
                workloads::Column::SecretVersions,
                Expr::value(serde_json::json!({ ABANDONED_KEY: attempts })),
            )
            .filter(workloads::Column::Id.eq(old.id))
            .exec(&self.db)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn ready_for(
        &self,
        id: Uuid,
        running: bool,
        reported_at: Option<chrono::NaiveDateTime>,
    ) -> Option<Duration> {
        let mut seen = self.ready_since.lock().unwrap_or_else(|e| e.into_inner());
        if !running {
            seen.remove(&id);
            return None;
        }
        let (since, first_report) = seen
            .entry(id)
            .or_insert_with(|| (Instant::now(), reported_at));
        if *first_report == reported_at {
            return Some(Duration::ZERO);
        }
        Some(since.elapsed())
    }

    fn forget_ready(&self, id: Uuid) {
        self.ready_since
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&id);
    }

    async fn switch_over(&self, replaced_id: Uuid, replacement_id: Uuid) -> Result<bool, String> {
        let transaction = self.db.begin().await.map_err(|e| e.to_string())?;
        let swapped = swap_identities(&transaction, replaced_id, replacement_id)
            .await
            .map_err(|e| e.to_string())?;
        let Some((old, new)) = swapped else {
            transaction.rollback().await.map_err(|e| e.to_string())?;
            return Ok(false);
        };
        transaction.commit().await.map_err(|e| e.to_string())?;
        self.forget_ready(replacement_id);
        for agent_id in [old.assigned_agent_id, new.assigned_agent_id]
            .into_iter()
            .flatten()
        {
            tokio::spawn(crate::services::gateway_notify::notify_assignment(agent_id));
        }
        if let Some(agent_id) = new.assigned_agent_id {
            notify_assigned(agent_id);
        }
        tracing::info!(workload = %replacement_id, replaced = %replaced_id, "replacement took over");
        Ok(true)
    }
}

async fn swap_identities(
    transaction: &impl ConnectionTrait,
    replaced_id: Uuid,
    replacement_id: Uuid,
) -> Result<Option<(workloads::Model, workloads::Model)>, DbErr> {
    let old = workloads::Entity::find_by_id(replaced_id)
        .lock_exclusive()
        .one(transaction)
        .await?;
    let new = workloads::Entity::find_by_id(replacement_id)
        .lock_exclusive()
        .one(transaction)
        .await?;
    let (Some(old), Some(new)) = (old, new) else {
        return Ok(None);
    };
    let valid = old.desired_state == DesiredState::Running.name()
        && new.desired_state == DesiredState::Running.name()
        && new.replaces_workload_id == Some(old.id)
        && new.status == WorkloadStatus::Running.name();
    if !valid || !agent_online(transaction, new.assigned_agent_id).await? {
        return Ok(None);
    }
    repoint(transaction, old.id, new.id).await?;
    repoint_records(transaction, &old, &new).await?;
    hand_over_name(transaction, &old, &new).await?;
    Ok(Some((old, new)))
}

async fn agent_online(
    transaction: &impl ConnectionTrait,
    agent_id: Option<Uuid>,
) -> Result<bool, DbErr> {
    let Some(agent_id) = agent_id else {
        return Ok(false);
    };
    let agent = agents::Entity::find_by_id(agent_id)
        .one(transaction)
        .await?;
    Ok(agent.is_some_and(|agent| agent.status == "Online"))
}

async fn hand_over_name(
    transaction: &impl ConnectionTrait,
    old: &workloads::Model,
    new: &workloads::Model,
) -> Result<(), DbErr> {
    workloads::Entity::update_many()
        .col_expr(
            workloads::Column::ServiceName,
            Expr::value(old.service_name.clone()),
        )
        .col_expr(
            workloads::Column::ReplacesWorkloadId,
            Expr::value(Option::<Uuid>::None),
        )
        .filter(workloads::Column::Id.eq(new.id))
        .exec(transaction)
        .await?;
    let retire_after = Utc::now().naive_utc() + chrono::Duration::seconds(RETIRE_GRACE_SECONDS);
    workloads::Entity::update_many()
        .col_expr(
            workloads::Column::ServiceName,
            Expr::value(Option::<String>::None),
        )
        .col_expr(workloads::Column::RetireAfter, Expr::value(retire_after))
        .filter(workloads::Column::Id.eq(old.id))
        .exec(transaction)
        .await?;
    Ok(())
}

async fn repoint(transaction: &impl ConnectionTrait, from: Uuid, to: Uuid) -> Result<(), DbErr> {
    ingress_routes::Entity::update_many()
        .col_expr(ingress_routes::Column::WorkloadId, Expr::value(to))
        .filter(ingress_routes::Column::WorkloadId.eq(from))
        .exec(transaction)
        .await?;
    network_members::Entity::update_many()
        .col_expr(network_members::Column::WorkloadId, Expr::value(to))
        .filter(network_members::Column::WorkloadId.eq(from))
        .exec(transaction)
        .await?;
    logs::Entity::update_many()
        .col_expr(logs::Column::WorkloadId, Expr::value(Some(to)))
        .filter(logs::Column::WorkloadId.eq(from))
        .exec(transaction)
        .await?;
    Ok(())
}

async fn repoint_records(
    transaction: &impl ConnectionTrait,
    old: &workloads::Model,
    new: &workloads::Model,
) -> Result<(), DbErr> {
    let (Some(group), Some(from), Some(to)) = (
        old.resource_group_id,
        old.guest_ip.as_ref(),
        new.guest_ip.as_ref(),
    ) else {
        return Ok(());
    };
    dns_records::Entity::update_many()
        .col_expr(dns_records::Column::Value, Expr::value(to.clone()))
        .filter(dns_records::Column::ResourceGroupId.eq(group))
        .filter(dns_records::Column::Value.eq(from.clone()))
        .exec(transaction)
        .await?;
    Ok(())
}

fn replacement_request(
    old: &workloads::Model,
    update: &UpdateWorkloadRequest,
) -> Result<CreateWorkloadRequest, String> {
    let current = old
        .env_vars
        .as_ref()
        .map(|stored| env_secrets::open_map(stored, &env_secrets::workload_env(old.id)))
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or_default();
    let env_vars = match &update.env_vars {
        Some(submitted) => env_secrets::restore_masked(submitted.clone(), &current),
        None => current,
    };
    let sizes = rightsized(old);
    Ok(CreateWorkloadRequest {
        name: old.name.clone(),
        image: update.image.clone().unwrap_or_else(|| old.image.clone()),
        cpu_millicores: sizes.cpu_millicores,
        memory_bytes: sizes.memory_bytes,
        disk_bytes: old.disk_bytes,
        env_vars: Some(env_vars),
        ports: update.ports.clone().or_else(|| typed_ports(old)),
        volume_mounts: None,
        bucket_bindings: None,
        resource_group_id: old.resource_group_id,
        stack_id: old.stack_id,
        service_name: None,
        restart_policy: update
            .restart_policy
            .unwrap_or_else(|| RestartPolicy::from_name(&old.restart_policy)),
        max_restarts: update.max_restarts.or(old.max_restarts),
        runtime_class: RuntimeClass::from_name(&old.runtime_class),
        cpu_limit_millicores: sizes.cpu_limit_millicores,
        memory_limit_bytes: sizes.memory_limit_bytes,
        auto_resources: old.auto_resources,
        replicas: None,
        spread_key: old.spread_key.clone(),
        topology_spread: TopologySpread::from_name(&old.topology_spread),
        vm_config: old
            .vm_config
            .as_ref()
            .and_then(|config| serde_json::from_value::<VmConfig>(config.clone()).ok())
            .map(|config| config.without_addresses()),
        replaces: None,
    })
}

#[cfg(test)]
mod tests;
