use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::models::workload::{AgentResources, CreateWorkloadRequest, CreateWorkloadResponse, WorkloadStatus};

pub struct SchedulerService {
    db: DatabaseConnection,
}

impl SchedulerService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn schedule(&self, req: CreateWorkloadRequest) -> Result<CreateWorkloadResponse, String> {
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

        match self.first_fit(&req, &agents) {
            Some(agent_id) => {
                crate::db::workloads::assign(&self.db, workload.id, agent_id)
                    .await
                    .map_err(|e| format!("Failed to assign workload: {}", e))?;

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
                    &format!("No suitable agent found workload_id={}", workload.id)
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

    fn first_fit(&self, req: &CreateWorkloadRequest, agents: &[AgentResources]) -> Option<Uuid> {
        let mut sorted: Vec<&AgentResources> = agents.iter().collect();
        sorted.sort_by(|a, b| {
            let score_a = a.free_cpu_millicores as i64 + a.free_memory_bytes / (1024 * 1024);
            let score_b = b.free_cpu_millicores as i64 + b.free_memory_bytes / (1024 * 1024);
            score_b.cmp(&score_a)
        });

        sorted
            .into_iter()
            .find(|a| {
                a.free_cpu_millicores >= req.cpu_millicores
                    && a.free_memory_bytes >= req.memory_bytes
                    && a.free_disk_bytes >= req.disk_bytes
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

    pub async fn delete_workload(&self, workload_id: Uuid) -> Result<(), String> {
        crate::db::workloads::delete(&self.db, workload_id)
            .await
            .map_err(|e| format!("Failed to delete workload: {}", e))
    }
}
