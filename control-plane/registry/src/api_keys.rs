use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// API Key für permanente Agent-Authentifizierung
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
        let id = Uuid::new_v4();
        let key = format!("csf_agent_{}", Uuid::new_v4().simple());

        Self {
            id,
            key,
            agent_id,
            created_at: Utc::now(),
            last_used: None,
            enabled: true,
        }
    }

    pub fn update_last_used(&mut self) {
        self.last_used = Some(Utc::now());
    }
}

/// Manager für API Keys
pub struct ApiKeyManager {
    db: DatabaseConnection,
}

impl ApiKeyManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Erstellt einen neuen API Key für einen Agent
    pub async fn create_key(&self, agent_id: Uuid) -> ApiKey {
        let api_key = ApiKey::new(agent_id);

        if let Err(e) =
            crate::db::create_api_key(&self.db, agent_id, api_key.key.clone()).await
        {
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

    /// Validiert einen API Key und updated last_used
    pub async fn validate_key(&self, key_str: &str) -> Result<Uuid, String> {
        match crate::db::get_agent_by_api_key(&self.db, key_str).await {
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

    /// Deaktiviert einen API Key
    pub async fn revoke_key(&self, _agent_id: Uuid) -> Result<(), String> {
        crate::log_warn!(
            "api_key_manager",
            "Key revocation not yet implemented with database"
        );
        Err("Not implemented".to_string())
    }

    /// Listet alle API Keys auf
    pub async fn list_keys(&self) -> Vec<ApiKey> {
        vec![]
    }

    /// Erneuert einen API Key für einen Agent
    pub async fn rotate_key(&self, agent_id: Uuid) -> Result<ApiKey, String> {
        self.revoke_key(agent_id).await?;
        let new_key = self.create_key(agent_id).await;
        crate::log_info!(
            "api_key_manager",
            &format!("Rotated API key for agent: {}", agent_id)
        );
        Ok(new_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_api_key_creation() {
        let manager = ApiKeyManager::new();
        let agent_id = Uuid::new_v4();
        let api_key = manager.create_key(agent_id).await;

        assert_eq!(api_key.agent_id, agent_id);
        assert!(api_key.enabled);
        assert!(api_key.last_used.is_none());
    }

    #[tokio::test]
    async fn test_api_key_validation() {
        let manager = ApiKeyManager::new();
        let agent_id = Uuid::new_v4();
        let api_key = manager.create_key(agent_id).await;

        let result = manager.validate_key(&api_key.key).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), agent_id);
    }

    #[tokio::test]
    async fn test_api_key_revocation() {
        let manager = ApiKeyManager::new();
        let agent_id = Uuid::new_v4();
        let api_key = manager.create_key(agent_id).await;

        let result = manager.revoke_key(agent_id).await;
        assert!(result.is_ok());

        let validation = manager.validate_key(&api_key.key).await;
        assert!(validation.is_err());
    }
}
