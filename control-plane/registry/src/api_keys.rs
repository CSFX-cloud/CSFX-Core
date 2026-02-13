use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
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
    keys: Arc<RwLock<HashMap<String, ApiKey>>>,
    agent_keys: Arc<RwLock<HashMap<Uuid, String>>>, // agent_id -> key mapping
}

impl ApiKeyManager {
    pub fn new() -> Self {
        Self {
            keys: Arc::new(RwLock::new(HashMap::new())),
            agent_keys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Erstellt einen neuen API Key für einen Agent
    pub async fn create_key(&self, agent_id: Uuid) -> ApiKey {
        let api_key = ApiKey::new(agent_id);
        let mut keys = self.keys.write().await;
        let mut agent_keys = self.agent_keys.write().await;

        keys.insert(api_key.key.clone(), api_key.clone());
        agent_keys.insert(agent_id, api_key.key.clone());

        crate::log_info!(
            "api_key_manager",
            &format!("✅ Created API key for agent: {}", agent_id)
        );

        api_key
    }

    /// Validiert einen API Key und updated last_used
    pub async fn validate_key(&self, key_str: &str) -> Result<Uuid, String> {
        let mut keys = self.keys.write().await;

        if let Some(key) = keys.get_mut(key_str) {
            if key.enabled {
                key.update_last_used();
                let agent_id = key.agent_id;

                crate::log_debug!(
                    "api_key_manager",
                    &format!("✅ API key validated for agent: {}", agent_id)
                );

                Ok(agent_id)
            } else {
                crate::log_warn!(
                    "api_key_manager",
                    &format!("❌ API key disabled: {}", key.id)
                );

                Err("API key is disabled".to_string())
            }
        } else {
            crate::log_warn!("api_key_manager", "❌ Invalid API key provided");

            Err("Invalid API key".to_string())
        }
    }

    /// Deaktiviert einen API Key
    pub async fn revoke_key(&self, agent_id: Uuid) -> Result<(), String> {
        let agent_keys = self.agent_keys.read().await;

        if let Some(key_str) = agent_keys.get(&agent_id) {
            let mut keys = self.keys.write().await;

            if let Some(key) = keys.get_mut(key_str) {
                key.enabled = false;

                crate::log_info!(
                    "api_key_manager",
                    &format!("🔒 Revoked API key for agent: {}", agent_id)
                );

                Ok(())
            } else {
                Err("Key not found".to_string())
            }
        } else {
            Err("No key found for agent".to_string())
        }
    }

    /// Listet alle API Keys auf
    pub async fn list_keys(&self) -> Vec<ApiKey> {
        let keys = self.keys.read().await;
        keys.values().cloned().collect()
    }

    /// Erneuert einen API Key für einen Agent
    pub async fn rotate_key(&self, agent_id: Uuid) -> Result<ApiKey, String> {
        // Revoke old key
        self.revoke_key(agent_id).await?;

        // Create new key
        let new_key = self.create_key(agent_id).await;

        crate::log_info!(
            "api_key_manager",
            &format!("🔄 Rotated API key for agent: {}", agent_id)
        );

        Ok(new_key)
    }
}

impl Default for ApiKeyManager {
    fn default() -> Self {
        Self::new()
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
