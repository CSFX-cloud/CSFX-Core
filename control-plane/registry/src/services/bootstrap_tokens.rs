use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use shared::agent_auth::hash_key;
use uuid::Uuid;

const TOKEN_PREFIX: &str = "csfx-bootstrap.";

pub const ROLE_MASTER: &str = "master";
pub const ROLE_WORKER: &str = "worker";

pub fn is_valid_role(role: &str) -> bool {
    role == ROLE_MASTER || role == ROLE_WORKER
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapToken {
    pub id: Uuid,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub token: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub max_uses: i32,
    pub use_count: i32,
    pub revoked: bool,
    pub role: String,
}

impl From<entity::bootstrap_tokens::Model> for BootstrapToken {
    fn from(model: entity::bootstrap_tokens::Model) -> Self {
        Self {
            id: model.id,
            token: String::new(),
            description: model.description,
            created_by: model.created_by,
            created_at: model.created_at.and_utc(),
            expires_at: model.expires_at.and_utc(),
            max_uses: model.max_uses,
            use_count: model.use_count,
            revoked: model.revoked,
            role: model.role,
        }
    }
}

pub struct BootstrapTokenManager {
    db: DatabaseConnection,
}

impl BootstrapTokenManager {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn is_bootstrap_token(token: &str) -> bool {
        token.starts_with(TOKEN_PREFIX)
    }

    pub async fn create(
        &self,
        description: Option<String>,
        created_by: String,
        ttl_hours: i64,
        max_uses: i32,
        role: String,
    ) -> Result<BootstrapToken, String> {
        let token = format!("{}{}", TOKEN_PREFIX, Uuid::new_v4().simple());
        let expires_at = Utc::now() + chrono::Duration::hours(ttl_hours);

        let model = crate::db::bootstrap_tokens::create(
            &self.db,
            hash_key(&token),
            description.clone(),
            created_by.clone(),
            expires_at,
            max_uses,
            role.clone(),
        )
        .await
        .map_err(|e| format!("Failed to create bootstrap token: {}", e))?;

        crate::log_info!(
            "bootstrap_token_manager",
            &format!(
                "Created bootstrap token max_uses={} ttl_hours={} role={} id={}",
                max_uses, ttl_hours, role, model.id
            )
        );

        Ok(BootstrapToken {
            token,
            ..model.into()
        })
    }

    pub async fn validate_and_use(&self, token_str: &str) -> Result<String, String> {
        let token_hash = hash_key(token_str);
        let consumed = crate::db::bootstrap_tokens::consume(&self.db, &token_hash)
            .await
            .map_err(|e| format!("Database error: {}", e))?;

        if let Some(consumed) = consumed {
            crate::log_info!(
                "bootstrap_token_manager",
                &format!(
                    "Bootstrap token used id={} use_count={}/{}",
                    consumed.id, consumed.use_count, consumed.max_uses
                )
            );
            return Ok(consumed.role);
        }
        Err(self.rejection(&token_hash).await)
    }

    async fn rejection(&self, token_hash: &str) -> String {
        let model = match crate::db::bootstrap_tokens::get_by_hash(&self.db, token_hash).await {
            Ok(Some(model)) => model,
            Ok(None) => return "Bootstrap token not found".to_string(),
            Err(e) => return format!("Database error: {}", e),
        };
        let (reason, message) = if model.revoked {
            ("revoked", "Bootstrap token has been revoked")
        } else if Utc::now().naive_utc() >= model.expires_at {
            ("expired", "Bootstrap token has expired")
        } else {
            ("exhausted", "Bootstrap token use limit reached")
        };
        crate::log_warn!(
            "bootstrap_token_manager",
            &format!("Rejected {} bootstrap token id={}", reason, model.id)
        );
        message.to_string()
    }

    pub async fn revoke(&self, id: Uuid) -> Result<(), String> {
        crate::db::bootstrap_tokens::revoke(&self.db, id)
            .await
            .map_err(|e| format!("Failed to revoke token: {}", e))?;

        crate::log_info!(
            "bootstrap_token_manager",
            &format!("Bootstrap token revoked id={}", id)
        );

        Ok(())
    }

    pub async fn list(&self) -> Vec<BootstrapToken> {
        match crate::db::bootstrap_tokens::get_all_active(&self.db).await {
            Ok(models) => models.into_iter().map(BootstrapToken::from).collect(),
            Err(e) => {
                crate::log_error!(
                    "bootstrap_token_manager",
                    &format!("Failed to list bootstrap tokens: {}", e)
                );
                vec![]
            }
        }
    }

    pub async fn cleanup_expired(&self) -> usize {
        match crate::db::bootstrap_tokens::delete_expired(&self.db).await {
            Ok(n) => {
                if n > 0 {
                    crate::log_info!(
                        "bootstrap_token_manager",
                        &format!("Cleaned up {} expired bootstrap tokens", n)
                    );
                }
                n as usize
            }
            Err(e) => {
                crate::log_error!(
                    "bootstrap_token_manager",
                    &format!("Failed to cleanup expired bootstrap tokens: {}", e)
                );
                0
            }
        }
    }
}
