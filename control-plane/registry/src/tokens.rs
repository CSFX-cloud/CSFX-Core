use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Registration Token - einmalig verwendbar für Agent-Registrierung
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationToken {
    pub id: Uuid,
    pub token: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub used_at: Option<DateTime<Utc>>,
    pub created_by: String,
    pub description: Option<String>,
}

impl RegistrationToken {
    pub fn new(description: Option<String>, created_by: String, ttl_hours: i64) -> Self {
        let id = Uuid::new_v4();
        let token = format!("reg_{}", Uuid::new_v4().simple());
        let now = Utc::now();

        Self {
            id,
            token,
            created_at: now,
            expires_at: now + chrono::Duration::hours(ttl_hours),
            used: false,
            used_at: None,
            created_by,
            description,
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

    /// Erstellt einen neuen Registration Token
    pub async fn create_token(
        &self,
        description: Option<String>,
        created_by: String,
        ttl_hours: i64,
    ) -> RegistrationToken {
        let token = RegistrationToken::new(description, created_by, ttl_hours);
        let mut tokens = self.tokens.write().await;
        tokens.insert(token.token.clone(), token.clone());

        crate::log_info!(
            "token_manager",
            &format!(
                "✅ Created registration token: {} (expires in {}h)",
                token.id, ttl_hours
            )
        );

        token
    }

    /// Validiert einen Token und markiert ihn als verwendet
    pub async fn validate_and_consume_token(&self, token_str: &str) -> Result<Uuid, String> {
        let mut tokens = self.tokens.write().await;

        if let Some(token) = tokens.get_mut(token_str) {
            if token.is_valid() {
                let token_id = token.id;
                token.mark_used();

                crate::log_info!(
                    "token_manager",
                    &format!("✅ Token validated and consumed: {}", token_id)
                );

                Ok(token_id)
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
        let token = manager
            .create_token(Some("Test token".to_string()), "admin".to_string(), 24)
            .await;

        assert!(!token.used);
        assert!(token.is_valid());
    }

    #[tokio::test]
    async fn test_token_validation() {
        let manager = TokenManager::new();
        let token = manager
            .create_token(Some("Test token".to_string()), "admin".to_string(), 24)
            .await;

        let result = manager.validate_and_consume_token(&token.token).await;
        assert!(result.is_ok());

        // Second attempt should fail
        let result2 = manager.validate_and_consume_token(&token.token).await;
        assert!(result2.is_err());
    }
}
