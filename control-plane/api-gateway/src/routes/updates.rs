use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::process::Command;

use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct VersionInfo {
    pub current_version: String,
    pub latest_version: String,
    pub update_available: bool,
    pub changelog: Option<String>,
    pub release_url: String,
    pub is_prerelease: bool,
    pub latest_beta_version: Option<String>,
    pub beta_release_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateRequest {
    pub version: String,
}

#[derive(Debug, Serialize)]
pub struct UpdateResponse {
    pub success: bool,
    pub message: String,
}

/// Get current version and check for updates
#[utoipa::path(
    get,
    path = "/api/updates/check",
    responses(
        (status = 200, description = "Version information retrieved successfully", body = VersionInfo),
        (status = 500, description = "Failed to check for updates")
    ),
    tag = "Updates"
)]
pub async fn check_updates(State(_state): State<AppState>) -> Result<Json<VersionInfo>, AppError> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    // Fetch latest release from GitHub
    let client = reqwest::Client::builder()
        .user_agent("CSF-Core-Updater")
        .build()
        .map_err(|e| AppError::InternalError(format!("Failed to create HTTP client: {}", e)))?;

    // Get latest stable release
    let response = client
        .get("https://api.github.com/repos/CS-Foundry/CSF-Core/releases/latest")
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to fetch releases: {}", e)))?;

    if !response.status().is_success() {
        return Err(AppError::InternalError(format!(
            "GitHub API returned status: {}",
            response.status()
        )));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse release data: {}", e)))?;

    let latest_version = release.tag_name.trim_start_matches('v').to_string();
    let update_available = version_compare(&current_version, &latest_version);

    // Check for beta releases
    let all_releases_response = client
        .get("https://api.github.com/repos/CS-Foundry/CSF-Core/releases")
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to fetch all releases: {}", e)))?;

    let all_releases: Vec<GitHubRelease> = all_releases_response
        .json()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse releases data: {}", e)))?;

    // Find latest beta release
    let latest_beta = all_releases.iter().find(|r| r.prerelease).map(|r| {
        (
            r.tag_name.trim_start_matches('v').to_string(),
            r.html_url.clone(),
        )
    });

    Ok(Json(VersionInfo {
        current_version,
        latest_version: latest_version.clone(),
        update_available,
        changelog: Some(release.body),
        release_url: release.html_url,
        is_prerelease: release.prerelease,
        latest_beta_version: latest_beta.as_ref().map(|(v, _)| v.clone()),
        beta_release_url: latest_beta.map(|(_, url)| url),
    }))
}

/// Trigger update installation
#[utoipa::path(
    post,
    path = "/api/updates/install",
    request_body = UpdateRequest,
    responses(
        (status = 200, description = "Update initiated successfully", body = UpdateResponse),
        (status = 400, description = "Invalid request"),
        (status = 500, description = "Failed to start update")
    ),
    tag = "Updates"
)]
pub async fn install_update(
    State(_state): State<AppState>,
    Json(payload): Json<UpdateRequest>,
) -> Result<Json<UpdateResponse>, AppError> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    // Safety check: don't downgrade
    if !version_compare(&current_version, &payload.version) {
        return Ok(Json(UpdateResponse {
            success: false,
            message: "Cannot install an older or same version".to_string(),
        }));
    }

    // Find the update script - try multiple locations
    let mut possible_paths: Vec<std::path::PathBuf> =
        vec![std::path::PathBuf::from("/opt/csf-core/scripts/update.sh")];

    if let Ok(dir) = std::env::current_dir() {
        possible_paths.push(dir.join("../scripts/update.sh"));
        possible_paths.push(dir.join("scripts/update.sh"));
    }

    let script_path = possible_paths
        .iter()
        .find(|&p: &&std::path::PathBuf| p.exists())
        .ok_or_else(|| {
            AppError::InternalError("Update script not found in any expected location".to_string())
        })?
        .clone();

    tracing::info!("Found update script at: {:?}", script_path);

    // Clone version for use in response message
    let version_for_message = payload.version.clone();

    // Start update process in background
    tokio::spawn(async move {
        match Command::new("sh")
            .arg(&script_path)
            .arg(&payload.version)
            .spawn()
        {
            Ok(_) => tracing::info!("Update process started for version {}", payload.version),
            Err(e) => tracing::error!("Failed to start update process: {}", e),
        }
    });

    Ok(Json(UpdateResponse {
        success: true,
        message: format!(
            "Update to version {} initiated. The application will restart shortly.",
            version_for_message
        ),
    }))
}

/// Get changelog for a specific version
#[utoipa::path(
    get,
    path = "/api/updates/changelog/{version}",
    params(
        ("version" = String, Path, description = "Version to get changelog for")
    ),
    responses(
        (status = 200, description = "Changelog retrieved successfully"),
        (status = 404, description = "Version not found"),
        (status = 500, description = "Failed to fetch changelog")
    ),
    tag = "Updates"
)]
pub async fn get_changelog(
    State(_state): State<AppState>,
    axum::extract::Path(version): axum::extract::Path<String>,
) -> Result<Json<String>, AppError> {
    let client = reqwest::Client::builder()
        .user_agent("CSF-Core-Updater")
        .build()
        .map_err(|e| AppError::InternalError(format!("Failed to create HTTP client: {}", e)))?;

    let url = format!(
        "https://api.github.com/repos/CS-Foundry/CSF-Core/releases/tags/v{}",
        version.trim_start_matches('v')
    );

    let response = client
        .get(&url)
        .send()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to fetch release: {}", e)))?;

    if response.status() == 404 {
        return Err(AppError::NotFound(format!("Version {} not found", version)));
    }

    if !response.status().is_success() {
        return Err(AppError::InternalError(format!(
            "GitHub API returned status: {}",
            response.status()
        )));
    }

    let release: GitHubRelease = response
        .json()
        .await
        .map_err(|e| AppError::InternalError(format!("Failed to parse release data: {}", e)))?;

    Ok(Json(release.body))
}

// Helper structs for GitHub API
#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    body: String,
    html_url: String,
    prerelease: bool,
}

// Compare versions (returns true if v2 is newer than v1)
fn version_compare(v1: &str, v2: &str) -> bool {
    let parse_version =
        |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse::<u32>().ok()).collect() };

    let v1_parts = parse_version(v1);
    let v2_parts = parse_version(v2);

    for i in 0..v1_parts.len().max(v2_parts.len()) {
        let p1 = v1_parts.get(i).copied().unwrap_or(0);
        let p2 = v2_parts.get(i).copied().unwrap_or(0);

        if p2 > p1 {
            return true;
        } else if p2 < p1 {
            return false;
        }
    }

    false
}

// Error handling
#[derive(Debug)]
pub enum AppError {
    InternalError(String),
    NotFound(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::InternalError(msg) => (StatusCode::INTERNAL_SERVER_ERROR, msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, msg),
        };

        (status, Json(serde_json::json!({ "error": message }))).into_response()
    }
}

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/updates/check", get(check_updates))
        .route("/updates/install", post(install_update))
        .route("/updates/changelog/:version", get(get_changelog))
}
