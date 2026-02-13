use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

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

impl ToString for AgentStatus {
    fn to_string(&self) -> String {
        match self {
            AgentStatus::Online => "online".to_string(),
            AgentStatus::Offline => "offline".to_string(),
            AgentStatus::Degraded => "degraded".to_string(),
        }
    }
}

/// Agent Registry Manager
pub struct AgentRegistry {
    agents: Arc<RwLock<HashMap<Uuid, RegisteredAgent>>>,
}

impl AgentRegistry {
    pub fn new() -> Self {
        Self {
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Registriert einen neuen Agent
    pub async fn register_agent(
        &self,
        name: String,
        hostname: String,
        os_type: String,
        os_version: String,
        architecture: String,
        agent_version: String,
        tags: Option<HashMap<String, String>>,
    ) -> RegisteredAgent {
        let agent = RegisteredAgent {
            id: Uuid::new_v4(),
            name: name.clone(),
            hostname,
            ip_address: None,
            os_type,
            os_version,
            architecture,
            agent_version,
            status: AgentStatus::Online,
            registered_at: Utc::now(),
            last_heartbeat: Some(Utc::now()),
            tags,
        };

        let mut agents = self.agents.write().await;
        agents.insert(agent.id, agent.clone());

        crate::log_info!(
            "agent_registry",
            &format!("✅ Registered new agent: {} ({})", name, agent.id)
        );

        agent
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
        let agent = registry
            .register_agent(
                "test-agent".to_string(),
                "test-host".to_string(),
                "linux".to_string(),
                "Ubuntu 22.04".to_string(),
                "x86_64".to_string(),
                "1.0.0".to_string(),
                None,
            )
            .await;

        assert_eq!(agent.name, "test-agent");
        assert_eq!(agent.status, AgentStatus::Online);
    }

    #[tokio::test]
    async fn test_agent_heartbeat() {
        let registry = AgentRegistry::new();
        let agent = registry
            .register_agent(
                "test-agent".to_string(),
                "test-host".to_string(),
                "linux".to_string(),
                "Ubuntu 22.04".to_string(),
                "x86_64".to_string(),
                "1.0.0".to_string(),
                None,
            )
            .await;

        let result = registry.update_heartbeat(agent.id).await;
        assert!(result.is_ok());
    }
}
