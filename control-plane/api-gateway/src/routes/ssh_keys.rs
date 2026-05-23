use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use entity::entities::{user_ssh_keys, UserSshKeys};
use sea_orm::{ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use base64::Engine as _;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::{auth::middleware::AuthenticatedUser, AppState};

#[derive(Deserialize)]
pub struct AddSshKeyRequest {
    pub name: String,
    pub public_key: String,
    pub expires_at: Option<chrono::DateTime<Utc>>,
}

#[derive(Serialize)]
pub struct SshKeyResponse {
    pub id: String,
    pub name: String,
    pub fingerprint: String,
    pub created_at: String,
    pub expires_at: Option<String>,
}

#[derive(Serialize)]
pub struct SshKeysForAgentResponse {
    pub keys: Vec<String>,
}

fn compute_fingerprint(public_key: &str) -> String {
    let key_part = public_key
        .split_whitespace()
        .nth(1)
        .unwrap_or(public_key);

    let decoded = base64::engine::general_purpose::STANDARD
        .decode(key_part)
        .unwrap_or_default();

    let mut hasher = Sha256::new();
    hasher.update(&decoded);
    let digest = hasher.finalize();
    let b64 = base64::engine::general_purpose::STANDARD.encode(digest);
    format!("SHA256:{}", b64)
}

pub async fn list_ssh_keys(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<SshKeyResponse>>, StatusCode> {
    let keys = UserSshKeys::find()
        .filter(user_ssh_keys::Column::UserId.eq(claims.user_id))
        .all(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let resp = keys
        .into_iter()
        .map(|k| SshKeyResponse {
            id: k.id.to_string(),
            name: k.name,
            fingerprint: k.fingerprint,
            created_at: k.created_at.to_string(),
            expires_at: k.expires_at.map(|t| t.to_string()),
        })
        .collect();

    Ok(Json(resp))
}

pub async fn add_ssh_key(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(payload): Json<AddSshKeyRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    let fingerprint = compute_fingerprint(&payload.public_key);

    let model = user_ssh_keys::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        user_id: ActiveValue::Set(claims.user_id),
        name: ActiveValue::Set(payload.name.clone()),
        public_key: ActiveValue::Set(payload.public_key.clone()),
        fingerprint: ActiveValue::Set(fingerprint.clone()),
        created_at: ActiveValue::Set(Utc::now().naive_utc()),
        expires_at: ActiveValue::Set(payload.expires_at.map(|t| t.naive_utc())),
    };

    let inserted = UserSshKeys::insert(model)
        .exec(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok((
        StatusCode::CREATED,
        Json(SshKeyResponse {
            id: inserted.last_insert_id.to_string(),
            name: payload.name,
            fingerprint,
            created_at: Utc::now().naive_utc().to_string(),
            expires_at: payload.expires_at.map(|t| t.naive_utc().to_string()),
        }),
    ))
}

pub async fn delete_ssh_key(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let key = UserSshKeys::find_by_id(id)
        .one(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    if key.user_id != claims.user_id {
        return Err(StatusCode::FORBIDDEN);
    }

    UserSshKeys::delete_by_id(id)
        .exec(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn get_keys_for_agent(
    State(state): State<AppState>,
    Path(agent_id): Path<String>,
) -> Result<Json<SshKeysForAgentResponse>, StatusCode> {
    let now = Utc::now().naive_utc();

    let all_keys = UserSshKeys::find()
        .all(&state.db_conn)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let keys: Vec<String> = all_keys
        .into_iter()
        .filter(|k| k.expires_at.map_or(true, |exp| exp > now))
        .map(|k| k.public_key)
        .collect();

    tracing::debug!(agent_id = %agent_id, count = keys.len(), "serving authorized keys");

    Ok(Json(SshKeysForAgentResponse { keys }))
}

pub fn ssh_keys_routes() -> Router<AppState> {
    Router::new()
        .route("/ssh-keys", get(list_ssh_keys))
        .route("/ssh-keys", post(add_ssh_key))
        .route("/ssh-keys/:id", delete(delete_ssh_key))
}

pub fn ssh_keys_internal_routes() -> Router<AppState> {
    Router::new()
        .route("/agents/:agent_id/authorized-keys", get(get_keys_for_agent))
}
