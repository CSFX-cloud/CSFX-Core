use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use shared::ssh_access::{self, SshAccess};

use crate::{auth::rbac::CanManageSystem, AppState};

const MAX_DURATION_HOURS: i64 = 24 * 365;

type ApiError = (StatusCode, Json<Value>);

#[derive(Debug, Serialize)]
pub struct SshAccessResponse {
    pub enabled: bool,
    pub expires_at: Option<DateTime<Utc>>,
    pub effective: bool,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSshAccessRequest {
    pub enabled: bool,
    pub duration_hours: Option<i64>,
}

impl From<SshAccess> for SshAccessResponse {
    fn from(access: SshAccess) -> Self {
        Self {
            effective: access.is_open(),
            enabled: access.enabled,
            expires_at: access.expires_at,
        }
    }
}

fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

fn internal(err: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %err, "ssh access request failed");
    error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

fn build_access(request: &UpdateSshAccessRequest) -> Result<SshAccess, ApiError> {
    if !request.enabled {
        return Ok(SshAccess::default());
    }
    let expires_at = match request.duration_hours {
        None => None,
        Some(hours) if (1..=MAX_DURATION_HOURS).contains(&hours) => {
            Some(Utc::now() + Duration::hours(hours))
        }
        Some(_) => {
            return Err(error(
                StatusCode::BAD_REQUEST,
                "duration_hours must be between 1 and 8760",
            ))
        }
    };
    Ok(SshAccess {
        enabled: true,
        expires_at,
    })
}

pub async fn get_ssh_access(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
) -> Result<Json<SshAccessResponse>, ApiError> {
    let access = ssh_access::load(&state.db_conn).await.map_err(internal)?;
    Ok(Json(access.into()))
}

pub async fn update_ssh_access(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    Json(request): Json<UpdateSshAccessRequest>,
) -> Result<Json<SshAccessResponse>, ApiError> {
    let access = build_access(&request)?;
    ssh_access::save(&state.db_conn, &access)
        .await
        .map_err(internal)?;
    tracing::info!(
        target: "csfx::audit",
        event = if access.enabled { "ssh.enabled" } else { "ssh.disabled" },
        user = %claims.username,
        expires_at = ?access.expires_at
    );
    Ok(Json(access.into()))
}

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/admin/settings/ssh",
        get(get_ssh_access).put(update_ssh_access),
    )
}
