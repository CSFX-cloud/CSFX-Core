use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
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
    db: DatabaseConnection,
}

impl TokenManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
        let token_obj = RegistrationToken::new(
            agent_id,
            expected_name.clone(),
            expected_hostname.clone(),
            description.clone(),
            created_by.clone(),
            ttl_hours,
        );

        if let Err(e) = crate::db::create_registry_token(
            &self.db,
            token_obj.token.clone(),
            description,
            created_by,
            token_obj.expires_at,
        )
        .await
        {
            crate::log_error!(
                "token_manager",
                &format!("Failed to save token to database: {}", e)
            );
        }

        crate::log_info!(
            "token_manager",
            &format!(
                "Created registration token for agent {} '{}@{}' (expires in {}h)",
                agent_id, expected_name, expected_hostname, ttl_hours
            )
        );

        token_obj
    }

    /// Validiert einen Token und markiert ihn als verwendet
    /// Gibt die Token-Daten zurück für weitere Validierung
    pub async fn validate_and_consume_token(
        &self,
        token_str: &str,
    ) -> Result<RegistrationToken, String> {
        let db_token = crate::db::get_registry_token_by_token(&self.db, token_str)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| "Token not found".to_string())?;

        if db_token.is_used {
            crate::log_warn!(
                "token_manager",
                &format!("Invalid token (already used): {}", db_token.id)
            );
            return Err("Token is already used".to_string());
        }

        if Utc::now().naive_utc() >= db_token.expires_at {
            crate::log_warn!(
                "token_manager",
                &format!("Invalid token (expired): {}", db_token.id)
            );
            return Err("Token is expired".to_string());
        }

        let token_data = RegistrationToken {
            id: db_token.id,
            token: db_token.token.clone(),
            agent_id: Uuid::nil(),
            created_at: db_token.created_at.and_utc(),
            expires_at: db_token.expires_at.and_utc(),
            used: db_token.is_used,
            used_at: db_token.used_at.map(|dt| dt.and_utc()),
            created_by: db_token.created_by.clone(),
            description: db_token.description.clone(),
            expected_name: String::new(),
            expected_hostname: String::new(),
        };

        crate::log_info!(
            "token_manager",
            &format!("Token validated: {}", db_token.id)
        );

        Ok(token_data)
    }

    /// Listet alle Tokens auf (für Admin-Interface)
    pub async fn list_tokens(&self) -> Vec<RegistrationToken> {
        match crate::db::get_unused_tokens(&self.db).await {
            Ok(db_tokens) => db_tokens
                .into_iter()
                .map(|t| RegistrationToken {
                    id: t.id,
                    token: t.token,
                    agent_id: Uuid::nil(),
                    created_at: t.created_at.and_utc(),
                    expires_at: t.expires_at.and_utc(),
                    used: t.is_used,
                    used_at: t.used_at.map(|dt| dt.and_utc()),
                    created_by: t.created_by,
                    description: t.description,
                    expected_name: String::new(),
                    expected_hostname: String::new(),
                })
                .collect(),
            Err(e) => {
                crate::log_error!(
                    "token_manager",
                    &format!("Failed to list tokens: {}", e)
                );
                vec![]
            }
        }
    }

    /// Löscht abgelaufene Tokens
    pub async fn cleanup_expired(&self) -> usize {
        match crate::db::delete_expired_tokens(&self.db).await {
            Ok(removed) => {
                if removed > 0 {
                    crate::log_info!(
                        "token_manager",
                        &format!("Cleaned up {} expired tokens", removed)
                    );
                }
                removed as usize
            }
            Err(e) => {
                crate::log_error!(
                    "token_manager",
                    &format!("Failed to cleanup expired tokens: {}", e)
                );
                0
            }
        }
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
