use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use uuid::Uuid;

use crate::db::agents::NewAgent;
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

        let proven_owner = machine_owner
            .as_ref()
            .filter(|owner| params.allow_reregister && params.proven_agent_id == Some(owner.id));
        if let Some(owner) = proven_owner {
            let agent = self.reregister(owner.id, params, mgmt_ipam).await?;
            return Ok((agent, true));
        }

        let agent = self
            .register_new(params, machine_owner.is_none(), mgmt_ipam)
            .await?;
        Ok((agent, false))
    }

    async fn reregister(
        &self,
        agent_id: Uuid,
        params: RegisterAgentParams,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<RegisteredAgent, String> {
        let db_agent = crate::db::agents::update_registration(
            &self.db,
            agent_id,
            params.agent_version,
            params.os_type,
            params.os_version,
            params.architecture,
            tags_json(params.tags.as_ref()),
        )
        .await
        .map_err(|e| format!("Failed to update existing agent: {}", e))?;

        crate::log_info!(
            "agent_registry",
            &format!(
                "Re-registered existing agent: {} id={}",
                params.name, db_agent.id
            )
        );

        let wg_tunnel_ip = match db_agent.wg_tunnel_ip.clone() {
            Some(ip) => ip,
            None => {
                self.assign_tunnel_ip(mgmt_ipam, db_agent.id, &db_agent.site_id)
                    .await?
            }
        };

        Ok(RegisteredAgent {
            tags: params.tags,
            wg_tunnel_ip: Some(wg_tunnel_ip),
            ..db_agent.into()
        })
    }

    async fn register_new(
        &self,
        params: RegisterAgentParams,
        machine_id_free: bool,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<RegisteredAgent, String> {
        let wg_tunnel_ip = mgmt_ipam
            .allocate(params.agent_id, DEFAULT_SITE)
            .await
            .map_err(|e| format!("Failed to allocate management tunnel IP: {}", e))?;

        let created = self
            .create_agent(&params, machine_id_free, &wg_tunnel_ip)
            .await;
        let db_agent = match created {
            Ok(db_agent) => db_agent,
            Err(e) => {
                release_tunnel_ip(mgmt_ipam, &wg_tunnel_ip).await;
                return Err(e);
            }
        };

        crate::log_info!(
            "agent_registry",
            &format!(
                "Registered new agent: {} id={} wg_tunnel_ip={}",
                params.name, db_agent.id, wg_tunnel_ip
            )
        );

        Ok(RegisteredAgent {
            tags: params.tags,
            ..db_agent.into()
        })
    }

    async fn create_agent(
        &self,
        params: &RegisterAgentParams,
        machine_id_free: bool,
        wg_tunnel_ip: &str,
    ) -> Result<entity::agents::Model, String> {
        let name = self.unique_name(&params.name, params.agent_id).await?;
        crate::db::agents::create(
            &self.db,
            NewAgent {
                id: params.agent_id,
                name,
                hostname: params.hostname.clone(),
                agent_version: params.agent_version.clone(),
                os_type: params.os_type.clone(),
                os_version: params.os_version.clone(),
                architecture: params.architecture.clone(),
                tags: tags_json(params.tags.as_ref()),
                wg_tunnel_ip: Some(wg_tunnel_ip.to_string()),
                role: params.role.clone(),
                machine_id: params.machine_id.clone().filter(|_| machine_id_free),
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
            release_tunnel_ip(mgmt_ipam, &ip).await;
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
        wg_public_key: Option<String>,
        wg_endpoint: Option<String>,
        agent_version: Option<String>,
        kvm_capable: bool,
        ssh_host_key: Option<String>,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<String, String> {
        let db_agent = crate::db::agents::update_heartbeat(
            &self.db,
            agent_id,
            "Online".to_string(),
            wg_public_key,
            wg_endpoint,
            agent_version,
            kvm_capable,
            ssh_host_key,
        )
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
            Ok(db_agent) => db_agent.map(RegisteredAgent::from),
            Err(_) => None,
        }
    }

    pub async fn list_agents(&self) -> Vec<RegisteredAgent> {
        match crate::db::agents::get_all(&self.db).await {
            Ok(db_agents) => db_agents.into_iter().map(RegisteredAgent::from).collect(),
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
            release_tunnel_ip(mgmt_ipam, &ip).await;
        }

        crate::log_info!(
            "agent_registry",
            &format!("Deregistered agent: {}", agent_id)
        );

        Ok(())
    }

    pub async fn get_statistics(&self) -> AgentStatistics {
        match crate::db::agents::get_statistics(&self.db).await {
            Ok(statistics) => statistics,
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

fn tags_json(tags: Option<&HashMap<String, String>>) -> Option<serde_json::Value> {
    tags.and_then(|tags| serde_json::to_value(tags).ok())
}

async fn release_tunnel_ip(mgmt_ipam: &MgmtIpamService, ip: &str) {
    if let Err(e) = mgmt_ipam.release(ip).await {
        crate::log_warn!(
            "agent_registry",
            &format!("Failed to release management tunnel IP {}: {}", ip, e)
        );
    }
}
