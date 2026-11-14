use chrono::{DateTime, Utc};
use sea_orm::{DatabaseConnection, TransactionTrait};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiKeyError {
    Invalid,
    Unavailable,
}

impl ApiKeyError {
    pub fn status(self) -> axum::http::StatusCode {
        match self {
            Self::Invalid => axum::http::StatusCode::UNAUTHORIZED,
            Self::Unavailable => axum::http::StatusCode::SERVICE_UNAVAILABLE,
        }
    }
}

impl std::fmt::Display for ApiKeyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Invalid => "Invalid API key",
            Self::Unavailable => "Database error",
        })
    }
}

pub struct ApiKeyManager {
    db: DatabaseConnection,
}

impl ApiKeyManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn issue_key(&self, agent_id: Uuid, revoke_existing: bool) -> anyhow::Result<ApiKey> {
        let api_key = ApiKey::new(agent_id);
        let key_hash = shared::agent_auth::hash_key(&api_key.key);

        let txn = self.db.begin().await?;
        if revoke_existing {
            let revoked = crate::db::api_keys::revoke_by_agent(&txn, agent_id).await?;
            crate::log_info!(
                "api_key_manager",
                &format!("Revoked {} API key(s) for agent: {}", revoked, agent_id)
            );
        }
        crate::db::api_keys::create(&txn, agent_id, key_hash).await?;
        txn.commit().await?;

        crate::log_info!(
            "api_key_manager",
            &format!("Created API key for agent: {}", agent_id)
        );

        Ok(api_key)
    }

    pub async fn validate_key(&self, key_str: &str) -> Result<Uuid, ApiKeyError> {
        match shared::agent_auth::agent_for_key(&self.db, key_str).await {
            Ok(Some(agent_id)) => {
                crate::log_debug!(
                    "api_key_manager",
                    &format!("API key validated for agent: {}", agent_id)
                );
                Ok(agent_id)
            }
            Ok(None) => {
                crate::log_warn!("api_key_manager", "Invalid API key provided");
                Err(ApiKeyError::Invalid)
            }
            Err(e) => {
                crate::log_error!(
                    "api_key_manager",
                    &format!("Failed to validate API key: {}", e)
                );
                Err(ApiKeyError::Unavailable)
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

#[cfg(test)]
mod tests {
    use super::ApiKeyError;
    use axum::http::StatusCode;

    #[test]
    fn database_failures_are_not_reported_as_invalid_keys() {
        assert_eq!(ApiKeyError::Invalid.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            ApiKeyError::Unavailable.status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
}
