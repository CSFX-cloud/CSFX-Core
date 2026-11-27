use chrono::{Duration, Utc};
use entity::bootstrap_tokens;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DbErr};
use uuid::Uuid;

use crate::agent_auth::hash_key;

pub const TOKEN_PREFIX: &str = "csfx-bootstrap.";

pub struct NewBootstrapToken {
    pub description: Option<String>,
    pub created_by: String,
    pub ttl_hours: i64,
    pub max_uses: i32,
    pub role: String,
}

pub async fn issue(
    db: &impl ConnectionTrait,
    request: NewBootstrapToken,
) -> Result<(String, bootstrap_tokens::Model), DbErr> {
    let token = format!("{TOKEN_PREFIX}{}", Uuid::new_v4().simple());
    let now = Utc::now();
    let model = bootstrap_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        token_hash: Set(hash_key(&token)),
        description: Set(request.description),
        created_by: Set(request.created_by),
        created_at: Set(now.naive_utc()),
        expires_at: Set((now + Duration::hours(request.ttl_hours)).naive_utc()),
        max_uses: Set(request.max_uses),
        use_count: Set(0),
        revoked: Set(false),
        revoked_at: Set(None),
        role: Set(request.role),
    }
    .insert(db)
    .await?;
    Ok((token, model))
}
