use axum::{extract::State, http::StatusCode, response::Json, routing::{get, post}, Router};
use etcd_client::Client;
use serde::{Deserialize, Serialize};
use std::env;

use crate::auth::crypto::{decrypt_secret, encrypt_secret};
use crate::auth::rbac::CanManageSystem;
use crate::AppState;

const ETCD_DESIRED_VERSION_KEY: &str = "/csf/config/desired_cp_version";
const ETCD_UPDATE_RESULT_KEY: &str = "/csf/config/last_update_result";
const ETCD_GHCR_TOKEN_KEY: &str = "/csf/config/ghcr_token";
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
    pub last_result: Option<String>,
    pub paused: bool,
    pub agent_version: Option<String>,
    pub updater_version: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GhcrTokenRequest {
    pub token: String,
    pub username: String,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system/update", post(trigger_update))
        .route("/system/update/status", get(update_status))
        .route("/system/update/pause", post(pause_updates))
        .route("/system/update/resume", post(resume_updates))
        .route("/system/ghcr-token", post(set_ghcr_token))
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
    if !is_valid_semver(&req.version) {
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

    client
        .put(ETCD_UPDATE_RESULT_KEY, b"in_progress", None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to write update result to etcd");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(version = %req.version, "update requested");

    spawn_update(req.version.clone());

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

    let desired = etcd_get(&mut client, ETCD_DESIRED_VERSION_KEY).await?;
    let last_result = etcd_get(&mut client, ETCD_UPDATE_RESULT_KEY).await?;
    let paused = etcd_get(&mut client, ETCD_PAUSED_KEY).await?.as_deref() == Some("true");

    let binary_dir = env::var("BINARY_DIR").unwrap_or_else(|_| "/usr/local/bin".to_string());
    let agent_version = binary_version(&format!("{}/csf-agent", binary_dir)).await;
    let updater_version = binary_version(&format!("{}/csf-updater", binary_dir)).await;

    Ok(Json(UpdateStatusResponse {
        current_version: env!("CARGO_PKG_VERSION").to_string(),
        desired_version: desired,
        last_result,
        paused,
        agent_version,
        updater_version,
    }))
}

async fn binary_version(path: &str) -> Option<String> {
    let output = tokio::process::Command::new(path)
        .arg("--version")
        .output()
        .await
        .ok()?;
    let raw = String::from_utf8(output.stdout).ok()?;
    raw.split_whitespace().last().map(|s| s.trim().to_string())
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

fn is_valid_semver(version: &str) -> bool {
    let v = version.strip_prefix('v').unwrap_or(version);
    let (base, _pre) = match v.split_once('-') {
        Some((b, p)) => (b, Some(p)),
        None => (v, None),
    };
    let parts: Vec<&str> = base.split('.').collect();
    parts.len() == 3 && parts.iter().all(|p| p.parse::<u32>().is_ok())
}

fn spawn_update(version: String) {
    tokio::spawn(async move {
        if let Err(e) = run_update(&version).await {
            tracing::error!(error = %e, version = %version, "update failed");
            write_result("failed").await;
        } else {
            tracing::info!(version = %version, "update completed");
            write_result("success").await;
        }
    });
}

async fn run_update(version: &str) -> Result<(), String> {
    let compose_file = env::var("COMPOSE_FILE")
        .unwrap_or_else(|_| "docker-compose.prod.yml".to_string());
    let ghcr_org = env::var("GHCR_ORG").map_err(|_| "GHCR_ORG not set".to_string())?;

    pull_images(&compose_file, &ghcr_org, version).await?;
    restart_services(&compose_file, &ghcr_org, version).await
}

async fn pull_images(compose_file: &str, ghcr_org: &str, version: &str) -> Result<(), String> {
    let status = tokio::process::Command::new("docker")
        .args(["compose", "-f", compose_file, "pull"])
        .env("GHCR_ORG", ghcr_org)
        .env("CSF_VERSION", version)
        .status()
        .await
        .map_err(|e| format!("docker compose pull failed: {}", e))?;

    if !status.success() {
        return Err(format!("docker compose pull exited with {}", status));
    }
    Ok(())
}

async fn restart_services(compose_file: &str, ghcr_org: &str, version: &str) -> Result<(), String> {
    let status = tokio::process::Command::new("docker")
        .args(["compose", "-f", compose_file, "up", "-d"])
        .env("GHCR_ORG", ghcr_org)
        .env("CSF_VERSION", version)
        .status()
        .await
        .map_err(|e| format!("docker compose up failed: {}", e))?;

    if !status.success() {
        return Err(format!("docker compose up exited with {}", status));
    }
    Ok(())
}

async fn write_result(result: &str) {
    if let Ok(mut client) = etcd_client().await {
        let _ = client.put(ETCD_UPDATE_RESULT_KEY, result.as_bytes(), None).await;
    }
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

async fn set_ghcr_token(
    _auth: CanManageSystem,
    State(_state): State<AppState>,
    Json(req): Json<GhcrTokenRequest>,
) -> Result<StatusCode, StatusCode> {
    if req.token.is_empty() || req.username.is_empty() {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }

    let encryption_key = env::var("SECRET_ENCRYPTION_KEY").map_err(|_| {
        tracing::error!("SECRET_ENCRYPTION_KEY not set");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let payload = format!("{}:{}", req.username, req.token);
    let encrypted = encrypt_secret(&payload, &encryption_key).map_err(|e| {
        tracing::error!(error = %e, "failed to encrypt ghcr token");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let mut client = etcd_client().await?;
    client
        .put(ETCD_GHCR_TOKEN_KEY, encrypted.as_bytes(), None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to write ghcr token to etcd");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    tracing::info!(username = %req.username, "ghcr token updated");
    Ok(StatusCode::NO_CONTENT)
}
