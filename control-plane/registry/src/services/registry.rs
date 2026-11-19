use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::agents::{Heartbeat, NewAgent, Registration};
use crate::models::agent::{AgentStatistics, PreRegisteredAgent, RegisteredAgent};
use crate::services::mgmt_ipam::{MgmtIpamService, DEFAULT_SITE};

pub struct RegisterAgentParams {
    pub agent_id: Uuid,
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub tags: Option<HashMap<String, String>>,
    pub allow_reregister: bool,
    pub proven_agent_id: Option<Uuid>,
    pub role: String,
    pub machine_id: Option<String>,
}

pub struct AgentRegistry {
    db: DatabaseConnection,
}

impl AgentRegistry {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn list_pending_agents(&self) -> Vec<PreRegisteredAgent> {
        match crate::db::tokens::get_unused(&self.db).await {
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
        match crate::db::tokens::delete_by_agent(&self.db, agent_id).await {
            Ok(0) => Err(format!(
                "No pending registration found for agent: {}",
                agent_id
            )),
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
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<(RegisteredAgent, bool), String> {
        let machine_owner = match params.machine_id.as_deref() {
            Some(machine_id) => crate::db::agents::get_by_machine_id(&self.db, machine_id)
                .await
                .map_err(|e| format!("Failed to query existing agent: {}", e))?,
            None => None,
        };

        let reregistered_id = machine_owner
            .as_ref()
            .filter(|owner| params.allow_reregister && params.proven_agent_id == Some(owner.id))
            .map(|owner| owner.id);

        match reregistered_id {
            Some(agent_id) => Ok((self.reregister(agent_id, params, mgmt_ipam).await?, true)),
            None => Ok((
                self.register_new(params, machine_owner.is_some(), mgmt_ipam)
                    .await?,
                false,
            )),
        }
    }

    async fn reregister(
        &self,
        agent_id: Uuid,
        params: RegisterAgentParams,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<RegisteredAgent, String> {
        let mut db_agent =
            crate::db::agents::update_registration(&self.db, agent_id, registration_of(&params))
                .await
                .map_err(|e| format!("Failed to update existing agent: {}", e))?;

        crate::log_info!(
            "agent_registry",
            &format!(
                "Re-registered existing agent: {} id={}",
                params.name, db_agent.id
            )
        );

        if db_agent.wg_tunnel_ip.is_none() {
            let ip = self
                .assign_tunnel_ip(mgmt_ipam, db_agent.id, &db_agent.site_id)
                .await?;
            db_agent.wg_tunnel_ip = Some(ip);
        }
        Ok(db_agent.into())
    }

    async fn register_new(
        &self,
        params: RegisterAgentParams,
        machine_id_taken: bool,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<RegisteredAgent, String> {
        let wg_tunnel_ip = mgmt_ipam
            .allocate(params.agent_id, DEFAULT_SITE)
            .await
            .map_err(|e| format!("Failed to allocate management tunnel IP: {}", e))?;

        let created = self
            .create_agent(&params, machine_id_taken, &wg_tunnel_ip)
            .await;
        let db_agent = match created {
            Ok(db_agent) => db_agent,
            Err(error) => {
                release_quietly(mgmt_ipam, &wg_tunnel_ip).await;
                return Err(error);
            }
        };

        crate::log_info!(
            "agent_registry",
            &format!(
                "Registered new agent: {} id={} wg_tunnel_ip={}",
                params.name, db_agent.id, wg_tunnel_ip
            )
        );

        Ok(db_agent.into())
    }

    async fn create_agent(
        &self,
        params: &RegisterAgentParams,
        machine_id_taken: bool,
        wg_tunnel_ip: &str,
    ) -> Result<entity::agents::Model, String> {
        let name = self.unique_name(&params.name, params.agent_id).await?;
        crate::db::agents::create(
            &self.db,
            NewAgent {
                id: params.agent_id,
                name,
                hostname: params.hostname.clone(),
                registration: registration_of(params),
                wg_tunnel_ip: Some(wg_tunnel_ip.to_string()),
                role: params.role.clone(),
                machine_id: params.machine_id.clone().filter(|_| !machine_id_taken),
            },
        )
        .await
        .map_err(|e| format!("Failed to create agent in database: {}", e))
    }

    async fn assign_tunnel_ip(
        &self,
        mgmt_ipam: &MgmtIpamService,
        agent_id: Uuid,
        site_id: &str,
    ) -> Result<String, String> {
        let ip = mgmt_ipam
            .allocate(agent_id, site_id)
            .await
            .map_err(|e| format!("Failed to allocate management tunnel IP: {}", e))?;
        if let Err(e) = crate::db::agents::set_wg_tunnel_ip(&self.db, agent_id, &ip).await {
            release_quietly(mgmt_ipam, &ip).await;
            return Err(format!("Failed to persist management tunnel IP: {}", e));
        }
        Ok(ip)
    }

    async fn unique_name(&self, name: &str, agent_id: Uuid) -> Result<String, String> {
        let taken = crate::db::agents::name_taken(&self.db, name)
            .await
            .map_err(|e| format!("Failed to query agent names: {}", e))?;
        if !taken {
            return Ok(name.to_string());
        }
        Ok(format!("{}-{}", name, &agent_id.simple().to_string()[..6]))
    }

    pub async fn update_heartbeat(
        &self,
        agent_id: Uuid,
        heartbeat: Heartbeat,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<String, String> {
        let db_agent = crate::db::agents::update_heartbeat(&self.db, agent_id, heartbeat)
            .await
            .map_err(|e| format!("Failed to update heartbeat: {}", e))?;

        if db_agent.wg_tunnel_ip.is_none() {
            let ip = self
                .assign_tunnel_ip(mgmt_ipam, agent_id, &db_agent.site_id)
                .await?;

            crate::log_info!(
                "agent_registry",
                &format!(
                    "Backfilled management tunnel IP agent={} ip={}",
                    agent_id, ip
                )
            );
        }

        crate::log_debug!(
            "agent_registry",
            &format!("Heartbeat received agent={}", agent_id)
        );

        Ok(db_agent.role)
    }

    pub async fn get_agent(&self, agent_id: Uuid) -> Option<RegisteredAgent> {
        match crate::db::agents::get_by_id(&self.db, agent_id).await {
            Ok(agent) => agent.map(Into::into),
            Err(_) => None,
        }
    }

    pub async fn list_agents(&self) -> Vec<RegisteredAgent> {
        match crate::db::agents::get_all(&self.db).await {
            Ok(db_agents) => db_agents.into_iter().map(Into::into).collect(),
            Err(e) => {
                crate::log_error!("agent_registry", &format!("Failed to list agents: {}", e));
                vec![]
            }
        }
    }

    pub async fn deregister_agent(
        &self,
        agent_id: Uuid,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<(), String> {
        let wg_tunnel_ip = crate::db::agents::get_by_id(&self.db, agent_id)
            .await
            .map_err(|e| format!("Failed to query agent: {}", e))?
            .and_then(|a| a.wg_tunnel_ip);

        crate::db::agents::delete(&self.db, agent_id)
            .await
            .map_err(|e| format!("Failed to delete agent: {}", e))?;

        if let Some(ip) = wg_tunnel_ip {
            if let Err(e) = mgmt_ipam.release(&ip).await {
                crate::log_warn!(
                    "agent_registry",
                    &format!("Failed to release management tunnel IP {}: {}", ip, e)
                );
            }
        }

        crate::log_info!(
            "agent_registry",
            &format!("Deregistered agent: {}", agent_id)
        );

        Ok(())
    }

    pub async fn get_statistics(&self) -> AgentStatistics {
        let counts = match crate::db::agents::count_by_status(&self.db).await {
            Ok(counts) => counts,
            Err(e) => {
                crate::log_error!(
                    "agent_registry",
                    &format!("Failed to get statistics: {}", e)
                );
                return AgentStatistics::default();
            }
        };
        let count = |status: &str| {
            counts
                .iter()
                .filter(|(name, _)| name == status)
                .map(|(_, total)| *total as usize)
                .sum()
        };
        AgentStatistics {
            total: counts.iter().map(|(_, total)| *total as usize).sum(),
            online: count("Online"),
            offline: count("Offline"),
            degraded: count("Degraded"),
        }
    }
}

fn registration_of(params: &RegisterAgentParams) -> Registration {
    Registration {
        agent_version: params.agent_version.clone(),
        os_type: params.os_type.clone(),
        os_version: params.os_version.clone(),
        architecture: params.architecture.clone(),
        tags: params
            .tags
            .as_ref()
            .and_then(|tags| serde_json::to_value(tags).ok()),
    }
}

async fn release_quietly(mgmt_ipam: &MgmtIpamService, ip: &str) {
    if let Err(e) = mgmt_ipam.release(ip).await {
        crate::log_warn!(
            "agent_registry",
            &format!("Failed to release management tunnel IP {}: {}", ip, e)
        );
    }
}
