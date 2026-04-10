use axum::{extract::State, http::StatusCode, response::Json, routing::{get, post}, Router};
use etcd_client::Client;
use serde::{Deserialize, Serialize};
use std::env;

use crate::auth::rbac::CanManageSystem;
use crate::AppState;

const ETCD_DESIRED_VERSION_KEY: &str = "/csf/config/desired_version";
const ETCD_AVAILABLE_FLAKE_REV_KEY: &str = "/csf/config/available_flake_rev";
const ETCD_DESIRED_FLAKE_REV_KEY: &str = "/csf/config/desired_flake_rev";
const ETCD_BUILD_STATUS_KEY: &str = "/csf/config/cp_build_status";
const ETCD_RESULT_KEY: &str = "/csf/config/last_build_result";
const ETCD_PAUSED_KEY: &str = "/csf/config/update_paused";

#[derive(Debug, Deserialize)]
pub struct UpdateRequest {
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    pub status: String,
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateStatusResponse {
    pub current_version: String,
    pub desired_version: Option<String>,
    pub available_flake_rev: Option<String>,
    pub desired_flake_rev: Option<String>,
    pub build_status: Option<String>,
    pub last_result: Option<String>,
    pub paused: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system/update", post(trigger_update))
        .route("/system/update/status", get(update_status))
        .route("/system/update/pause", post(pause_updates))
        .route("/system/update/resume", post(resume_updates))
}

async fn etcd_client() -> Result<Client, StatusCode> {
    let endpoints = env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| "http://etcd:2379".to_string());

    Client::connect([endpoints.as_str()], None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to connect to etcd");
            StatusCode::SERVICE_UNAVAILABLE
        })
}

async fn trigger_update(
    _auth: CanManageSystem,
    State(_state): State<AppState>,
    Json(req): Json<UpdateRequest>,
) -> Result<Json<UpdateResponse>, StatusCode> {
    if !is_valid_version(&req.version) {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let mut client = etcd_client().await?;

    client
        .put(ETCD_DESIRED_VERSION_KEY, req.version.as_bytes(), None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to write desired version to etcd");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(version = %req.version, "update requested");

    Ok(Json(UpdateResponse {
        status: "update_scheduled".to_string(),
        version: req.version,
    }))
}

async fn update_status(
    _auth: CanManageSystem,
    State(_state): State<AppState>,
) -> Result<Json<UpdateStatusResponse>, StatusCode> {
    let mut client = etcd_client().await?;

    let desired_version = etcd_get(&mut client, ETCD_DESIRED_VERSION_KEY).await?;
    let available_flake_rev = etcd_get(&mut client, ETCD_AVAILABLE_FLAKE_REV_KEY).await?;
    let desired_flake_rev = etcd_get(&mut client, ETCD_DESIRED_FLAKE_REV_KEY).await?;
    let build_status = etcd_get(&mut client, ETCD_BUILD_STATUS_KEY).await?;
    let last_result = etcd_get(&mut client, ETCD_RESULT_KEY).await?;
    let paused = etcd_get(&mut client, ETCD_PAUSED_KEY).await?.as_deref() == Some("true");

    Ok(Json(UpdateStatusResponse {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        desired_version,
        available_flake_rev,
        desired_flake_rev,
        build_status,
        last_result,
        paused,
    }))
}

async fn etcd_get(client: &mut Client, key: &str) -> Result<Option<String>, StatusCode> {
    let resp = client.get(key, None).await.map_err(|e| {
        tracing::error!(error = %e, key = key, "failed to read from etcd");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(resp
        .kvs()
        .first()
        .and_then(|kv| std::str::from_utf8(kv.value()).ok())
        .map(|s| s.to_string()))
}

fn is_valid_version(version: &str) -> bool {
    let v = version.trim_start_matches('v');
    !v.is_empty() && v.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
}

async fn pause_updates(
    _auth: CanManageSystem,
    State(_state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let mut client = etcd_client().await?;
    client
        .put(ETCD_PAUSED_KEY, b"true", None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to set update_paused in etcd");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::info!("updates paused");
    Ok(StatusCode::NO_CONTENT)
}

async fn resume_updates(
    _auth: CanManageSystem,
    State(_state): State<AppState>,
) -> Result<StatusCode, StatusCode> {
    let mut client = etcd_client().await?;
    client
        .delete(ETCD_PAUSED_KEY, None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete update_paused from etcd");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    tracing::info!("updates resumed");
    Ok(StatusCode::NO_CONTENT)
}
