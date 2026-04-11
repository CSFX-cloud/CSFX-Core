use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKey {
    pub id: Uuid,
    pub key: String,
    pub agent_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_used: Option<DateTime<Utc>>,
    pub enabled: bool,
}

impl ApiKey {
    pub fn new(agent_id: Uuid) -> Self {
        Self {
            id: Uuid::new_v4(),
            key: format!("csfx_agent_{}", Uuid::new_v4().simple()),
            agent_id,
            created_at: Utc::now(),
            last_used: None,
            enabled: true,
        }
    }
}

pub struct ApiKeyManager {
    db: DatabaseConnection,
}

impl ApiKeyManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create_key(&self, agent_id: Uuid) -> ApiKey {
        let api_key = ApiKey::new(agent_id);
        let key_hash = crate::db::api_keys::hash_key(&api_key.key);

        if let Err(e) = crate::db::api_keys::create(&self.db, agent_id, key_hash).await {
            crate::log_error!(
                "api_key_manager",
                &format!("Failed to save API key to database: {}", e)
            );
        }

        crate::log_info!(
            "api_key_manager",
            &format!("Created API key for agent: {}", agent_id)
        );

        api_key
    }

    pub async fn validate_key(&self, key_str: &str) -> Result<Uuid, String> {
        match crate::db::api_keys::get_agent_by_key(&self.db, key_str).await {
            Ok(Some(agent)) => {
                crate::log_debug!(
                    "api_key_manager",
                    &format!("API key validated for agent: {}", agent.id)
                );
                Ok(agent.id)
            }
            Ok(None) => {
                crate::log_warn!("api_key_manager", "Invalid API key provided");
                Err("Invalid API key".to_string())
            }
            Err(e) => {
                crate::log_error!(
                    "api_key_manager",
                    &format!("Failed to validate API key: {}", e)
                );
                Err("Database error".to_string())
            }
        }
    }

    pub async fn revoke_key(&self, agent_id: Uuid) -> Result<(), String> {
        match crate::db::api_keys::revoke_by_agent(&self.db, agent_id).await {
            Ok(revoked) => {
                crate::log_info!(
                    "api_key_manager",
                    &format!("Revoked {} API key(s) for agent: {}", revoked, agent_id)
                );
                Ok(())
            }
            Err(e) => {
                crate::log_error!(
                    "api_key_manager",
                    &format!("Failed to revoke API key for agent {}: {}", agent_id, e)
                );
                Err(format!("Failed to revoke key: {}", e))
            }
        }
    }
}
