use entity::entities::workloads;
use shared::vm::VmConfig;
use uuid::Uuid;

use crate::models::workload::CreateWorkloadRequest;
use crate::services::guest_ip;
use crate::services::scheduler::{SchedulerService, REPLACEMENT_EXISTS};

const GUEST_IP_ATTEMPTS: usize = 5;

fn is_replacement_conflict(error: &sea_orm::DbErr) -> bool {
    error
        .to_string()
        .contains("workloads_replaces_workload_id_key")
}

fn is_unique_violation(error: &sea_orm::DbErr) -> bool {
    matches!(
        error.sql_err(),
        Some(sea_orm::SqlErr::UniqueConstraintViolation(_))
    )
}

impl SchedulerService {
    pub(crate) async fn create_with_guest_ip(
        &self,
        req: &CreateWorkloadRequest,
    ) -> Result<workloads::Model, String> {
        let Some(group) = req.resource_group_id else {
            if req
                .vm_config
                .as_ref()
                .is_some_and(VmConfig::requests_addresses)
            {
                return Err("ip addresses require a resource group".to_string());
            }
            return crate::db::workloads::create(&self.db, req, None)
                .await
                .map_err(|e| format!("Failed to persist workload: {}", e));
        };

        for _ in 0..GUEST_IP_ATTEMPTS {
            let ips = self.assign_guest_ips(group, req.vm_config.as_ref()).await?;
            let mut stored = req.clone();
            stored.vm_config = req
                .vm_config
                .as_ref()
                .map(|config| config.with_assigned_ips(&ips));
            match crate::db::workloads::create(&self.db, &stored, ips.first().cloned()).await {
                Ok(workload) => return Ok(workload),
                Err(e) if is_replacement_conflict(&e) => return Err(REPLACEMENT_EXISTS.to_string()),
                Err(e) if is_unique_violation(&e) => continue,
                Err(e) => return Err(format!("Failed to persist workload: {}", e)),
            }
        }
        Err(format!(
            "Could not reserve a guest ip in resource group {}",
            group
        ))
    }

    async fn assign_guest_ips(
        &self,
        resource_group_id: Uuid,
        config: Option<&VmConfig>,
    ) -> Result<Vec<String>, String> {
        let cidr = crate::db::resource_groups::internal_cidr(&self.db, resource_group_id)
            .await
            .map_err(|e| format!("Failed to fetch resource group cidr: {}", e))?
            .ok_or_else(|| format!("Resource group {} not found", resource_group_id))?;
        let used = crate::db::workloads::used_guest_ips(&self.db, resource_group_id)
            .await
            .map_err(|e| format!("Failed to fetch used guest ips: {}", e))?;
        let requested = config
            .map(VmConfig::requested_ips)
            .unwrap_or_else(|| vec![None]);

        guest_ip::assign(&cidr, &used, &requested)
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
            let guest_ip = self.assign_guest_ips(group, None).await?.remove(0);
            crate::db::workloads::set_guest_ip(&self.db, workload.id, guest_ip)
                .await
                .map_err(|e| format!("Failed to store guest ip: {}", e))?;
            assigned += 1;
        }
        Ok(assigned)
    }
}
