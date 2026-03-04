use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationToken {
    pub id: Uuid,
    pub token: String,
    pub agent_id: Uuid,
    pub expected_name: String,
    pub expected_hostname: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub used: bool,
    pub used_at: Option<DateTime<Utc>>,
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
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            token: format!("reg_{}_{}", agent_id.simple(), Uuid::new_v4().simple()),
            agent_id,
            expected_name,
            expected_hostname,
            description,
            created_by,
            created_at: now,
            expires_at: now + chrono::Duration::hours(ttl_hours),
            used: false,
            used_at: None,
        }
    }
}

pub struct TokenManager {
    db: DatabaseConnection,
}

impl TokenManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

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

        if let Err(e) = crate::db::tokens::create(
            &self.db,
            agent_id,
            token_obj.token.clone(),
            expected_name.clone(),
            expected_hostname.clone(),
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

    pub async fn validate_and_consume_token(
        &self,
        token_str: &str,
    ) -> Result<RegistrationToken, String> {
        let db_token = crate::db::tokens::get_by_token(&self.db, token_str)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| "Token not found".to_string())?;

        if db_token.is_used {
            crate::log_warn!(
                "token_manager",
                &format!("Rejected already-used token: {}", db_token.id)
            );
            return Err("Token is already used".to_string());
        }

        if Utc::now().naive_utc() >= db_token.expires_at {
            crate::log_warn!(
                "token_manager",
                &format!("Rejected expired token: {}", db_token.id)
            );
            return Err("Token is expired".to_string());
        }

        let agent_id = db_token
            .agent_id
            .ok_or_else(|| "Token has no associated agent".to_string())?;

        let token_data = RegistrationToken {
            id: db_token.id,
            token: db_token.token.clone(),
            agent_id,
            expected_name: db_token.expected_name.clone(),
            expected_hostname: db_token.expected_hostname.clone(),
            description: db_token.description.clone(),
            created_by: db_token.created_by.clone(),
            created_at: db_token.created_at.and_utc(),
            expires_at: db_token.expires_at.and_utc(),
            used: false,
            used_at: None,
        };

        crate::db::tokens::mark_used(&self.db, db_token.id, agent_id)
            .await
            .map_err(|e| format!("Failed to consume token: {}", e))?;

        crate::log_info!(
            "token_manager",
            &format!("Token consumed: {} agent={}", db_token.id, agent_id)
        );

        Ok(token_data)
    }

    pub async fn list_tokens(&self) -> Vec<RegistrationToken> {
        match crate::db::tokens::get_unused(&self.db).await {
            Ok(db_tokens) => db_tokens
                .into_iter()
                .filter_map(|t| {
                    let agent_id = t.agent_id?;
                    Some(RegistrationToken {
                        id: t.id,
                        token: t.token,
                        agent_id,
                        expected_name: t.expected_name,
                        expected_hostname: t.expected_hostname,
                        description: t.description,
                        created_by: t.created_by,
                        created_at: t.created_at.and_utc(),
                        expires_at: t.expires_at.and_utc(),
                        used: t.is_used,
                        used_at: t.used_at.map(|dt| dt.and_utc()),
                    })
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

    pub async fn cleanup_expired(&self) -> usize {
        match crate::db::tokens::delete_expired(&self.db).await {
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
