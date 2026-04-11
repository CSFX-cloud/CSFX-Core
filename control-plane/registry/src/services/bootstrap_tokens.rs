use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const TOKEN_PREFIX: &str = "csfx-bootstrap.";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BootstrapToken {
    pub id: Uuid,
    pub token: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub max_uses: i32,
    pub use_count: i32,
    pub revoked: bool,
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
    ) -> Result<BootstrapToken, String> {
        let token = format!("{}{}", TOKEN_PREFIX, Uuid::new_v4().simple());
        let expires_at = Utc::now() + chrono::Duration::hours(ttl_hours);

        let model = crate::db::bootstrap_tokens::create(
            &self.db,
            token.clone(),
            description.clone(),
            created_by.clone(),
            expires_at,
            max_uses,
        )
        .await
        .map_err(|e| format!("Failed to create bootstrap token: {}", e))?;

        crate::log_info!(
            "bootstrap_token_manager",
            &format!(
                "Created bootstrap token max_uses={} ttl_hours={} id={}",
                max_uses, ttl_hours, model.id
            )
        );

        Ok(BootstrapToken {
            id: model.id,
            token: model.token,
            description: model.description,
            created_by: model.created_by,
            created_at: model.created_at.and_utc(),
            expires_at: model.expires_at.and_utc(),
            max_uses: model.max_uses,
            use_count: model.use_count,
            revoked: model.revoked,
        })
    }

    pub async fn validate_and_use(&self, token_str: &str) -> Result<(), String> {
        let model = crate::db::bootstrap_tokens::get_by_token(&self.db, token_str)
            .await
            .map_err(|e| format!("Database error: {}", e))?
            .ok_or_else(|| "Bootstrap token not found".to_string())?;

        if model.revoked {
            crate::log_warn!(
                "bootstrap_token_manager",
                &format!("Rejected revoked bootstrap token id={}", model.id)
            );
            return Err("Bootstrap token has been revoked".to_string());
        }

        if Utc::now().naive_utc() >= model.expires_at {
            crate::log_warn!(
                "bootstrap_token_manager",
                &format!("Rejected expired bootstrap token id={}", model.id)
            );
            return Err("Bootstrap token has expired".to_string());
        }

        if model.use_count >= model.max_uses {
            crate::log_warn!(
                "bootstrap_token_manager",
                &format!(
                    "Rejected exhausted bootstrap token id={} use_count={} max_uses={}",
                    model.id, model.use_count, model.max_uses
                )
            );
            return Err("Bootstrap token use limit reached".to_string());
        }

        crate::db::bootstrap_tokens::increment_use_count(&self.db, model.id)
            .await
            .map_err(|e| format!("Failed to increment use count: {}", e))?;

        crate::log_info!(
            "bootstrap_token_manager",
            &format!(
                "Bootstrap token used id={} use_count={}/{}",
                model.id,
                model.use_count + 1,
                model.max_uses
            )
        );

        Ok(())
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
            Ok(models) => models
                .into_iter()
                .map(|m| BootstrapToken {
                    id: m.id,
                    token: m.token,
                    description: m.description,
                    created_by: m.created_by,
                    created_at: m.created_at.and_utc(),
                    expires_at: m.expires_at.and_utc(),
                    max_uses: m.max_uses,
                    use_count: m.use_count,
                    revoked: m.revoked,
                })
                .collect(),
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
