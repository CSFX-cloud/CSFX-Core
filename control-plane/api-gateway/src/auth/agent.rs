use crate::AppState;
use axum::{
    extract::FromRequestParts,
    http::{request::Parts, StatusCode},
};
use entity::entities::{agent_api_keys, agents};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub struct AgentApiKey {
    pub agent_id: Uuid,
}

fn hash_key(key: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

impl FromRequestParts<AppState> for AgentApiKey {
    type Rejection = StatusCode;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let raw_key = parts
            .headers
            .get("X-API-Key")
            .and_then(|v| v.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;

        let key_hash = hash_key(raw_key);

        let key_record = agent_api_keys::Entity::find()
            .filter(agent_api_keys::Column::KeyHash.eq(&key_hash))
            .filter(agent_api_keys::Column::IsActive.eq(true))
            .one(&state.agent_db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        agents::Entity::find_by_id(key_record.agent_id)
            .one(&state.agent_db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::UNAUTHORIZED)?;

        Ok(AgentApiKey {
            agent_id: key_record.agent_id,
        })
    }
}
