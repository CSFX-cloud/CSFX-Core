use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Registration Token - einmalig verwendbar für Agent-Registrierung
/// Jetzt agent-spezifisch: Token ist an einen Pre-Registered Agent gebunden
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationToken {
    pub id: Uuid,
    pub token: String,
    pub agent_id: Uuid, // Token ist an spezifischen Agent gebunden
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub used_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub description: Option<String>,
    // Expected values für Validierung
    pub expected_name: String,
    pub expected_hostname: String,
}

impl RegistrationToken {
    pub fn new(
        agent_id: Uuid,
        expected_name: String,
        expected_hostname: String,
        description: Option<String>,
        created_by: String,
        ttl_hours: i64,
    ) -> Self {
        let id = Uuid::new_v4();
        let token = format!("reg_{}_{}", agent_id.simple(), Uuid::new_v4().simple());
        let now = Utc::now();

        Self {
            id,
            token,
            agent_id,
            created_at: now,
            expires_at: now + chrono::Duration::hours(ttl_hours),
            used: false,
            used_at: None,
            created_by,
            description,
            expected_name,
            expected_hostname,
        }
    }

    pub fn is_valid(&self) -> bool {
        !self.used && Utc::now() < self.expires_at
    }

    pub fn mark_used(&mut self) {
        self.used = true;
        self.used_at = Some(Utc::now());
    }
}

/// Manager für Registration Tokens
pub struct TokenManager {
    tokens: Arc<RwLock<HashMap<String, RegistrationToken>>>,
}

impl TokenManager {
    pub fn new() -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Erstellt einen neuen agent-spezifischen Registration Token
    pub async fn create_token(
        &self,
        agent_id: Uuid,
        expected_name: String,
        expected_hostname: String,
        description: Option<String>,
        created_by: String,
        ttl_hours: i64,
    ) -> RegistrationToken {
        let token = RegistrationToken::new(
            agent_id,
            expected_name.clone(),
            expected_hostname.clone(),
            description,
            created_by,
            ttl_hours,
        );
        let mut tokens = self.tokens.write().await;
        tokens.insert(token.token.clone(), token.clone());

        crate::log_info!(
            "token_manager",
            &format!(
                "✅ Created registration token for agent {} '{}@{}' (expires in {}h)",
                agent_id, expected_name, expected_hostname, ttl_hours
            )
        );

        token
    }

    /// Validiert einen Token und markiert ihn als verwendet
    /// Gibt die Token-Daten zurück für weitere Validierung
    pub async fn validate_and_consume_token(
        &self,
        token_str: &str,
    ) -> Result<RegistrationToken, String> {
        let mut tokens = self.tokens.write().await;

        if let Some(token) = tokens.get_mut(token_str) {
            if token.is_valid() {
                let token_copy = token.clone();
                token.mark_used();

                crate::log_info!(
                    "token_manager",
                    &format!(
                        "✅ Token validated and consumed: {} for agent {}",
                        token_copy.id, token_copy.agent_id
                    )
                );

                Ok(token_copy)
            } else {
                let reason = if token.used {
                    "already used"
                } else {
                    "expired"
                };

                crate::log_warn!(
                    "token_manager",
                    &format!("❌ Invalid token ({}): {}", reason, token.id)
                );

                Err(format!("Token is {}", reason))
            }
        } else {
            crate::log_warn!(
                "token_manager",
                &format!("❌ Token not found: {}", token_str)
            );

            Err("Token not found".to_string())
        }
    }

    /// Listet alle Tokens auf (für Admin-Interface)
    pub async fn list_tokens(&self) -> Vec<RegistrationToken> {
        let tokens = self.tokens.read().await;
        tokens.values().cloned().collect()
    }

    /// Löscht abgelaufene Tokens
    pub async fn cleanup_expired(&self) -> usize {
        let mut tokens = self.tokens.write().await;
        let before_count = tokens.len();

        tokens.retain(|_, token| {
            let is_expired = Utc::now() >= token.expires_at;
            !is_expired || !token.used
        });

        let removed = before_count - tokens.len();

        if removed > 0 {
            crate::log_info!(
                "token_manager",
                &format!("🧹 Cleaned up {} expired tokens", removed)
            );
        }

        removed
    }
}

impl Default for TokenManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_token_creation() {
        let manager = TokenManager::new();
        let agent_id = Uuid::new_v4();
        let token = manager
            .create_token(
                agent_id,
                "test-agent".to_string(),
                "test-host".to_string(),
                Some("Test token".to_string()),
                "admin".to_string(),
                24,
            )
            .await;

        assert!(!token.used);
        assert!(token.is_valid());
        assert_eq!(token.agent_id, agent_id);
    }

    #[tokio::test]
    async fn test_token_validation() {
        let manager = TokenManager::new();
        let agent_id = Uuid::new_v4();
        let token = manager
            .create_token(
                agent_id,
                "test-agent".to_string(),
                "test-host".to_string(),
                Some("Test token".to_string()),
                "admin".to_string(),
                24,
            )
            .await;

        let result = manager.validate_and_consume_token(&token.token).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().agent_id, agent_id);

        // Second attempt should fail
        let result2 = manager.validate_and_consume_token(&token.token).await;
        assert!(result2.is_err());
    }
}
