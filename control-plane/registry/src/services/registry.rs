use chrono::{NaiveDateTime, Utc};
use sea_orm::DatabaseConnection;
use std::collections::HashMap;
use uuid::Uuid;

use crate::models::agent::{AgentStatistics, AgentStatus, PreRegisteredAgent, RegisteredAgent};
use crate::services::mgmt_ipam::MgmtIpamService;

pub struct PreRegisterParams {
    pub agent_id: Uuid,
    pub name: String,
    pub hostname: String,
    pub expected_os_type: Option<String>,
    pub expected_architecture: Option<String>,
    pub tags: Option<HashMap<String, String>>,
    pub created_by: String,
    pub registration_token: String,
    pub token_expires_at: chrono::DateTime<Utc>,
}

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
    pub role: String,
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
        let tags_json = params
            .tags
            .as_ref()
            .and_then(|t| serde_json::to_value(t).ok());

        if params.allow_reregister {
            if let Some(existing) = crate::db::agents::get_by_hostname(&self.db, &params.hostname)
                .await
                .map_err(|e| format!("Failed to query existing agent: {}", e))?
            {
                let db_agent = crate::db::agents::update_registration(
                    &self.db,
                    existing.id,
                    params.agent_version.clone(),
                    params.os_type.clone(),
                    params.os_version.clone(),
                    params.architecture.clone(),
                    tags_json,
                    None,
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

                let wg_tunnel_ip = match db_agent.wg_tunnel_ip {
                    Some(ip) => Some(ip),
                    None => {
                        let ip = mgmt_ipam.allocate(db_agent.id).await.map_err(|e| {
                            format!("Failed to allocate management tunnel IP: {}", e)
                        })?;
                        crate::db::agents::set_wg_tunnel_ip(&self.db, db_agent.id, &ip)
                            .await
                            .map_err(|e| {
                                format!("Failed to persist management tunnel IP: {}", e)
                            })?;
                        Some(ip)
                    }
                };

                let agent = RegisteredAgent {
                    id: db_agent.id,
                    name: db_agent.name,
                    hostname: db_agent.hostname,
                    ip_address: db_agent.ip_address,
                    os_type: db_agent.os_type,
                    os_version: db_agent.os_version,
                    architecture: db_agent.architecture,
                    agent_version: db_agent.agent_version,
                    status: AgentStatus::Online,
                    registered_at: db_agent.registered_at.and_utc(),
                    last_heartbeat: db_agent
                        .last_heartbeat
                        .map(|dt: NaiveDateTime| dt.and_utc()),
                    tags: params.tags,
                    wg_tunnel_ip,
                };

                return Ok((agent, true));
            }
        }

        let wg_tunnel_ip = mgmt_ipam
            .allocate(params.agent_id)
            .await
            .map_err(|e| format!("Failed to allocate management tunnel IP: {}", e))?;

        let db_agent = crate::db::agents::create(
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
            None,
            Some(wg_tunnel_ip.clone()),
            params.role.clone(),
        )
        .await
        .map_err(|e| format!("Failed to create agent in database: {}", e))?;

        crate::log_info!(
            "agent_registry",
            &format!(
                "Registered new agent: {} id={} wg_tunnel_ip={}",
                params.name, db_agent.id, wg_tunnel_ip
            )
        );

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
            last_heartbeat: db_agent
                .last_heartbeat
                .map(|dt: NaiveDateTime| dt.and_utc()),
            tags: params.tags,
            wg_tunnel_ip: db_agent.wg_tunnel_ip,
        };

        Ok((agent, false))
    }

    pub async fn update_heartbeat(
        &self,
        agent_id: Uuid,
        wg_public_key: Option<String>,
        wg_endpoint: Option<String>,
        wg_tunnel_ip: Option<String>,
        agent_version: Option<String>,
        kvm_capable: bool,
        mgmt_ipam: &MgmtIpamService,
    ) -> Result<(), String> {
        let db_agent = crate::db::agents::update_heartbeat(
            &self.db,
            agent_id,
            "Online".to_string(),
            wg_public_key,
            wg_endpoint,
            wg_tunnel_ip,
            agent_version,
            kvm_capable,
        )
        .await
        .map_err(|e| format!("Failed to update heartbeat: {}", e))?;

        if db_agent.wg_tunnel_ip.is_none() {
            let ip = mgmt_ipam
                .allocate(agent_id)
                .await
                .map_err(|e| format!("Failed to backfill management tunnel IP: {}", e))?;
            crate::db::agents::set_wg_tunnel_ip(&self.db, agent_id, &ip)
                .await
                .map_err(|e| format!("Failed to persist management tunnel IP: {}", e))?;

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

        Ok(())
    }

    pub async fn get_agent(&self, agent_id: Uuid) -> Option<RegisteredAgent> {
        match crate::db::agents::get_by_id(&self.db, agent_id).await {
            Ok(Some(db_agent)) => Some(RegisteredAgent {
                id: db_agent.id,
                name: db_agent.name,
                hostname: db_agent.hostname,
                ip_address: db_agent.ip_address,
                os_type: db_agent.os_type,
                os_version: db_agent.os_version,
                architecture: db_agent.architecture,
                agent_version: db_agent.agent_version,
                status: AgentStatus::from_str(&db_agent.status),
                registered_at: db_agent.registered_at.and_utc(),
                last_heartbeat: db_agent
                    .last_heartbeat
                    .map(|dt: NaiveDateTime| dt.and_utc()),
                tags: None,
                wg_tunnel_ip: db_agent.wg_tunnel_ip,
            }),
            _ => None,
        }
    }

    pub async fn list_agents(&self) -> Vec<RegisteredAgent> {
        match crate::db::agents::get_all(&self.db).await {
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
                    status: AgentStatus::from_str(&db_agent.status),
                    registered_at: db_agent.registered_at.and_utc(),
                    last_heartbeat: db_agent
                        .last_heartbeat
                        .map(|dt: NaiveDateTime| dt.and_utc()),
                    tags: None,
                    wg_tunnel_ip: db_agent.wg_tunnel_ip,
                })
                .collect(),
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

    pub async fn check_agent_health(&self, degraded_seconds: i64, offline_seconds: i64) -> usize {
        if let Err(e) =
            crate::db::agents::mark_degraded_by_timeout(&self.db, degraded_seconds).await
        {
            crate::log_error!("agent_registry", &format!("Failed to mark degraded: {}", e));
        }
        match crate::db::agents::mark_offline_by_timeout(&self.db, offline_seconds).await {
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
        match crate::db::agents::get_statistics(&self.db).await {
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
