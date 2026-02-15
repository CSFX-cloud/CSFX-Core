use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Pre-Registrierter Agent - wartet auf erste Verbindung
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
    pub registration_token: String, // Token für Registrierung
    pub token_expires_at: DateTime<Utc>,
}

/// Registrierter Agent
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

/// Parameter für Agent Pre-Registration
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
}

/// Parameter für Agent Registrierung
#[derive(Debug, Clone)]
pub struct RegisterAgentParams {
    pub agent_id: Uuid, // Von Pre-Registration übernommen
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub tags: Option<HashMap<String, String>>,
}

/// Agent Registry Manager
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<Uuid, RegisteredAgent>>>,
    pre_registered: Arc<RwLock<HashMap<Uuid, PreRegisteredAgent>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
            pre_registered: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Pre-Registriert einen Agent (Admin-Vorgang)
    pub async fn pre_register_agent(&self, params: PreRegisterParams) -> PreRegisteredAgent {
        let pre_agent = PreRegisteredAgent {
            id: Uuid::new_v4(),
            name: params.name.clone(),
            hostname: params.hostname.clone(),
            expected_os_type: params.expected_os_type,
            expected_architecture: params.expected_architecture,
            tags: params.tags,
            created_at: Utc::now(),
            created_by: params.created_by,
            registration_token: params.registration_token.clone(),
            token_expires_at: params.token_expires_at,
        };

        let mut pre_registered = self.pre_registered.write().await;
        pre_registered.insert(pre_agent.id, pre_agent.clone());

        crate::log_info!(
            "agent_registry",
            &format!(
                "📝 Pre-registered agent: {} '{}@{}' (ID: {})",
                params.name, params.name, params.hostname, pre_agent.id
            )
        );

        pre_agent
    }

    /// Holt einen pre-registrierten Agent nach ID
    pub async fn get_pre_registered_agent(&self, agent_id: Uuid) -> Option<PreRegisteredAgent> {
        let pre_registered = self.pre_registered.read().await;
        pre_registered.get(&agent_id).cloned()
    }

    /// Listet alle pre-registrierten Agents auf (pending)
    pub async fn list_pending_agents(&self) -> Vec<PreRegisteredAgent> {
        let pre_registered = self.pre_registered.read().await;
        pre_registered.values().cloned().collect()
    }

    /// Löscht einen pre-registrierten Agent
    pub async fn delete_pre_registered_agent(&self, agent_id: Uuid) -> Result<(), String> {
        let mut pre_registered = self.pre_registered.write().await;

        if pre_registered.remove(&agent_id).is_some() {
            crate::log_info!(
                "agent_registry",
                &format!("🗑️  Deleted pre-registered agent: {}", agent_id)
            );
            Ok(())
        } else {
            Err("Pre-registered agent not found".to_string())
        }
    }

    /// Registriert einen neuen Agent (mit Pre-Registration Validierung)
    pub async fn register_agent(
        &self,
        params: RegisterAgentParams,
    ) -> Result<RegisteredAgent, String> {
        // Entferne aus Pre-Registration
        let mut pre_registered = self.pre_registered.write().await;
        pre_registered.remove(&params.agent_id);
        drop(pre_registered); // Release lock

        let agent = RegisteredAgent {
            id: params.agent_id,
            name: params.name.clone(),
            hostname: params.hostname,
            ip_address: None,
            os_type: params.os_type,
            os_version: params.os_version,
            architecture: params.architecture,
            agent_version: params.agent_version,
            status: AgentStatus::Online,
            registered_at: Utc::now(),
            last_heartbeat: Some(Utc::now()),
            tags: params.tags,
        };

        let mut agents = self.agents.write().await;
        agents.insert(agent.id, agent.clone());

        crate::log_info!(
            "agent_registry",
            &format!("✅ Registered new agent: {} ({})", params.name, agent.id)
        );

        Ok(agent)
    }

    /// Updated den Heartbeat eines Agents
    pub async fn update_heartbeat(&self, agent_id: Uuid) -> Result<(), String> {
        let mut agents = self.agents.write().await;

        if let Some(agent) = agents.get_mut(&agent_id) {
            agent.last_heartbeat = Some(Utc::now());
            agent.status = AgentStatus::Online;

            crate::log_debug!(
                "agent_registry",
                &format!("💓 Heartbeat received from agent: {}", agent_id)
            );

            Ok(())
        } else {
            crate::log_warn!(
                "agent_registry",
                &format!("❌ Heartbeat from unknown agent: {}", agent_id)
            );

            Err("Agent not found".to_string())
        }
    }

    /// Holt einen Agent nach ID
    pub async fn get_agent(&self, agent_id: Uuid) -> Option<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents.get(&agent_id).cloned()
    }

    /// Listet alle Agents auf
    pub async fn list_agents(&self) -> Vec<RegisteredAgent> {
        let agents = self.agents.read().await;
        agents.values().cloned().collect()
    }

    /// Entfernt einen Agent
    pub async fn deregister_agent(&self, agent_id: Uuid) -> Result<(), String> {
        let mut agents = self.agents.write().await;

        if agents.remove(&agent_id).is_some() {
            crate::log_info!(
                "agent_registry",
                &format!("🗑️  Deregistered agent: {}", agent_id)
            );

            Ok(())
        } else {
            Err("Agent not found".to_string())
        }
    }

    /// Markiert inaktive Agents als offline
    pub async fn check_agent_health(&self, timeout_seconds: i64) -> usize {
        let mut agents = self.agents.write().await;
        let now = Utc::now();
        let mut marked_offline = 0;

        for agent in agents.values_mut() {
            if let Some(last_heartbeat) = agent.last_heartbeat {
                let duration = now.signed_duration_since(last_heartbeat);

                if duration.num_seconds() > timeout_seconds && agent.status != AgentStatus::Offline
                {
                    agent.status = AgentStatus::Offline;
                    marked_offline += 1;

                    crate::log_warn!(
                        "agent_registry",
                        &format!("⚠️  Agent marked as offline: {} ({})", agent.name, agent.id)
                    );
                }
            }
        }

        if marked_offline > 0 {
            crate::log_info!(
                "agent_registry",
                &format!("🔍 Health check: {} agents marked offline", marked_offline)
            );
        }

        marked_offline
    }

    /// Statistiken über registrierte Agents
    pub async fn get_statistics(&self) -> AgentStatistics {
        let agents = self.agents.read().await;
        let total = agents.len();
        let mut online = 0;
        let mut offline = 0;
        let mut degraded = 0;

        for agent in agents.values() {
            match agent.status {
                AgentStatus::Online => online += 1,
                AgentStatus::Offline => offline += 1,
                AgentStatus::Degraded => degraded += 1,
            }
        }

        AgentStatistics {
            total,
            online,
            offline,
            degraded,
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

impl Default for AgentRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_agent_registration() {
        let registry = AgentRegistry::new();
        let agent_id = Uuid::new_v4();
        let agent = registry
            .register_agent(RegisterAgentParams {
                agent_id,
                name: "test-agent".to_string(),
                hostname: "test-host".to_string(),
                os_type: "linux".to_string(),
                os_version: "Ubuntu 22.04".to_string(),
                architecture: "x86_64".to_string(),
                agent_version: "1.0.0".to_string(),
                tags: None,
            })
            .await
            .unwrap();

        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.status, AgentStatus::Online);
        assert_eq!(agent.id, agent_id);
    }

    #[tokio::test]
    async fn test_agent_heartbeat() {
        let registry = AgentRegistry::new();
        let agent_id = Uuid::new_v4();
        let agent = registry
            .register_agent(RegisterAgentParams {
                agent_id,
                name: "test-agent".to_string(),
                hostname: "test-host".to_string(),
                os_type: "linux".to_string(),
                os_version: "Ubuntu 22.04".to_string(),
                architecture: "x86_64".to_string(),
                agent_version: "1.0.0".to_string(),
                tags: None,
            })
            .await
            .unwrap();

        let result = registry.update_heartbeat(agent.id).await;
        assert!(result.is_ok());
    }
}
