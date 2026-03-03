use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreRegisteredAgent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub expected_os_type: Option<String>,
    pub expected_architecture: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub created_at: DateTime<Utc>,
    pub created_by: String,
    pub registration_token: String,
    pub token_expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisteredAgent {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub status: AgentStatus,
    pub registered_at: DateTime<Utc>,
    pub last_heartbeat: Option<DateTime<Utc>>,
    pub tags: Option<HashMap<String, String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum AgentStatus {
    Online,
    Offline,
    Degraded,
}

impl std::fmt::Display for AgentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentStatus::Online => write!(f, "online"),
            AgentStatus::Offline => write!(f, "offline"),
            AgentStatus::Degraded => write!(f, "degraded"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct PreRegisterParams {
    pub name: String,
    pub hostname: String,
    pub expected_os_type: Option<String>,
    pub expected_architecture: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub created_by: String,
    pub registration_token: String,
    pub token_expires_at: DateTime<Utc>,
    pub agent_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct RegisterAgentParams {
    pub agent_id: Uuid,
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub tags: Option<HashMap<String, String>>,
}

pub struct AgentRegistry {
    db: DatabaseConnection,
}

impl AgentRegistry {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn pre_register_agent(&self, params: PreRegisterParams) -> PreRegisteredAgent {
        let pre_agent = PreRegisteredAgent {
            id: params.agent_id,
            name: params.name.clone(),
            hostname: params.hostname.clone(),
            expected_os_type: params.expected_os_type,
            expected_architecture: params.expected_architecture,
            tags: params.tags,
            created_at: Utc::now(),
            created_by: params.created_by,
            registration_token: params.registration_token,
            token_expires_at: params.token_expires_at,
        };

        crate::log_info!(
            "agent_registry",
            &format!(
                "Pre-registered agent: '{}@{}' id={}",
                params.name, params.hostname, params.agent_id
            )
        );

        pre_agent
    }

    pub async fn list_pending_agents(&self) -> Vec<PreRegisteredAgent> {
        match crate::db::get_unused_tokens(&self.db).await {
            Ok(tokens) => tokens
                .into_iter()
                .filter_map(|t| {
                    let agent_id = t.agent_id?;
                    Some(PreRegisteredAgent {
                        id: agent_id,
                        name: t.expected_name.clone(),
                        hostname: t.expected_hostname.clone(),
                        expected_os_type: None,
                        expected_architecture: None,
                        tags: None,
                        created_at: t.created_at.and_utc(),
                        created_by: t.created_by,
                        registration_token: t.token,
                        token_expires_at: t.expires_at.and_utc(),
                    })
                })
                .collect(),
            Err(e) => {
                crate::log_error!(
                    "agent_registry",
                    &format!("Failed to list pending agents: {}", e)
                );
                vec![]
            }
        }
    }

    pub async fn delete_pre_registered_agent(&self, agent_id: Uuid) -> Result<(), String> {
        match crate::db::delete_pending_token_by_agent(&self.db, agent_id).await {
            Ok(0) => Err(format!("No pending registration found for agent: {}", agent_id)),
            Ok(_) => {
                crate::log_info!(
                    "agent_registry",
                    &format!("Deleted pending registration for agent: {}", agent_id)
                );
                Ok(())
            }
            Err(e) => Err(format!("Failed to delete pending registration: {}", e)),
        }
    }

    pub async fn register_agent(
        &self,
        params: RegisterAgentParams,
    ) -> Result<RegisteredAgent, String> {
        let tags_json = params
            .tags
            .as_ref()
            .and_then(|t| serde_json::to_value(t).ok());

        let db_agent = crate::db::create_agent(
            &self.db,
            params.agent_id,
            params.name.clone(),
            params.hostname.clone(),
            None,
            params.agent_version.clone(),
            params.os_type.clone(),
            params.os_version.clone(),
            params.architecture.clone(),
            "Online".to_string(),
            tags_json,
            None,
        )
        .await
        .map_err(|e| format!("Failed to create agent in database: {}", e))?;

        let agent = RegisteredAgent {
            id: db_agent.id,
            name: db_agent.name.clone(),
            hostname: db_agent.hostname,
            ip_address: db_agent.ip_address,
            os_type: db_agent.os_type,
            os_version: db_agent.os_version,
            architecture: db_agent.architecture,
            agent_version: db_agent.agent_version,
            status: AgentStatus::Online,
            registered_at: db_agent.registered_at.and_utc(),
            last_heartbeat: db_agent.last_heartbeat.map(|dt| dt.and_utc()),
            tags: params.tags,
        };

        crate::log_info!(
            "agent_registry",
            &format!("Registered new agent: {} id={}", params.name, agent.id)
        );

        Ok(agent)
    }

    pub async fn update_heartbeat(&self, agent_id: Uuid) -> Result<(), String> {
        crate::db::update_agent_heartbeat(&self.db, agent_id, "Online".to_string())
            .await
            .map_err(|e| format!("Failed to update heartbeat: {}", e))?;

        crate::log_debug!(
            "agent_registry",
            &format!("Heartbeat received agent={}", agent_id)
        );

        Ok(())
    }

    pub async fn get_agent(&self, agent_id: Uuid) -> Option<RegisteredAgent> {
        match crate::db::get_agent_by_id(&self.db, agent_id).await {
            Ok(Some(db_agent)) => Some(RegisteredAgent {
                id: db_agent.id,
                name: db_agent.name,
                hostname: db_agent.hostname,
                ip_address: db_agent.ip_address,
                os_type: db_agent.os_type,
                os_version: db_agent.os_version,
                architecture: db_agent.architecture,
                agent_version: db_agent.agent_version,
                status: match db_agent.status.as_str() {
                    "Online" => AgentStatus::Online,
                    "Offline" => AgentStatus::Offline,
                    "Degraded" => AgentStatus::Degraded,
                    _ => AgentStatus::Offline,
                },
                registered_at: db_agent.registered_at.and_utc(),
                last_heartbeat: db_agent.last_heartbeat.map(|dt| dt.and_utc()),
                tags: None,
            }),
            _ => None,
        }
    }

    pub async fn list_agents(&self) -> Vec<RegisteredAgent> {
        match crate::db::get_all_agents(&self.db).await {
            Ok(db_agents) => db_agents
                .into_iter()
                .map(|db_agent| RegisteredAgent {
                    id: db_agent.id,
                    name: db_agent.name,
                    hostname: db_agent.hostname,
                    ip_address: db_agent.ip_address,
                    os_type: db_agent.os_type,
                    os_version: db_agent.os_version,
                    architecture: db_agent.architecture,
                    agent_version: db_agent.agent_version,
                    status: match db_agent.status.as_str() {
                        "Online" => AgentStatus::Online,
                        "Offline" => AgentStatus::Offline,
                        "Degraded" => AgentStatus::Degraded,
                        _ => AgentStatus::Offline,
                    },
                    registered_at: db_agent.registered_at.and_utc(),
                    last_heartbeat: db_agent.last_heartbeat.map(|dt| dt.and_utc()),
                    tags: None,
                })
                .collect(),
            Err(e) => {
                crate::log_error!(
                    "agent_registry",
                    &format!("Failed to list agents: {}", e)
                );
                vec![]
            }
        }
    }

    pub async fn deregister_agent(&self, agent_id: Uuid) -> Result<(), String> {
        crate::db::delete_agent(&self.db, agent_id)
            .await
            .map_err(|e| format!("Failed to delete agent: {}", e))?;

        crate::log_info!(
            "agent_registry",
            &format!("Deregistered agent: {}", agent_id)
        );

        Ok(())
    }

    pub async fn check_agent_health(&self, timeout_seconds: i64) -> usize {
        match crate::db::update_agents_offline(&self.db, timeout_seconds).await {
            Ok(marked_offline) => {
                if marked_offline > 0 {
                    crate::log_info!(
                        "agent_registry",
                        &format!("Health check: {} agents marked offline", marked_offline)
                    );
                }
                marked_offline as usize
            }
            Err(e) => {
                crate::log_error!(
                    "agent_registry",
                    &format!("Failed to check agent health: {}", e)
                );
                0
            }
        }
    }

    pub async fn get_statistics(&self) -> AgentStatistics {
        match crate::db::get_agent_statistics(&self.db).await {
            Ok((total, online, offline, degraded)) => AgentStatistics {
                total,
                online,
                offline,
                degraded,
            },
            Err(e) => {
                crate::log_error!(
                    "agent_registry",
                    &format!("Failed to get statistics: {}", e)
                );
                AgentStatistics {
                    total: 0,
                    online: 0,
                    offline: 0,
                    degraded: 0,
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatistics {
    pub total: usize,
    pub online: usize,
    pub offline: usize,
    pub degraded: usize,
}
