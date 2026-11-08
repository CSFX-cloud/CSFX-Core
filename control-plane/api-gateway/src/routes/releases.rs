use axum::{extract::Query, http::StatusCode, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};

use crate::auth::rbac::CanManageSystem;
use crate::AppState;

const RELEASES_API: &str = "https://api.github.com/repos/CSFX-cloud/CSFX-Core/releases";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Deserialize)]
struct GithubRelease {
    tag_name: String,
    prerelease: bool,
    html_url: String,
    name: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ReleaseEntry {
    pub version: String,
    pub tag: String,
    pub prerelease: bool,
    pub html_url: String,
    pub name: Option<String>,
    pub is_current: bool,
    pub is_newer: bool,
}

#[derive(Debug, Serialize)]
pub struct ReleasesResponse {
    pub current_version: String,
    pub update_available: bool,
    pub latest_update: Option<String>,
    pub releases: Vec<ReleaseEntry>,
}

#[derive(Debug, Deserialize)]
pub struct ReleasesQuery {
    #[serde(default)]
    pub include_pre: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/system/releases", get(list_releases))
}

async fn list_releases(
    _auth: CanManageSystem,
    Query(query): Query<ReleasesQuery>,
) -> Result<Json<ReleasesResponse>, StatusCode> {
    let github_releases = fetch_github_releases().await?;
    let include_pre = query.include_pre || is_prerelease_version(CURRENT_VERSION);

    let releases: Vec<ReleaseEntry> = github_releases
        .iter()
        .filter(|r| include_pre || !r.prerelease)
        .take(10)
        .map(|r| {
            let version = r.tag_name.trim_start_matches('v').to_string();
            let is_current = version == CURRENT_VERSION;
            let is_newer = semver_is_newer(&version, CURRENT_VERSION);
            ReleaseEntry {
                version,
                tag: r.tag_name.clone(),
                prerelease: r.prerelease,
                html_url: r.html_url.clone(),
                name: r.name.clone(),
                is_current,
                is_newer,
            }
        })
        .collect();

    let latest_update = releases
        .iter()
        .find(|r| r.is_newer)
        .map(|r| r.version.clone());

    let update_available = latest_update.is_some();

    Ok(Json(ReleasesResponse {
        current_version: CURRENT_VERSION.to_string(),
        update_available,
        latest_update,
        releases,
    }))
}

async fn fetch_github_releases() -> Result<Vec<GithubRelease>, StatusCode> {
    let client = reqwest::Client::new();
    let resp = client
        .get(RELEASES_API)
        .header("User-Agent", "csfx-api-gateway")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "github releases fetch failed");
            StatusCode::BAD_GATEWAY
        })?;

    if !resp.status().is_success() {
        tracing::error!(status = %resp.status(), "github api returned error");
        return Err(StatusCode::BAD_GATEWAY);
    }

    resp.json::<Vec<GithubRelease>>().await.map_err(|e| {
        tracing::error!(error = %e, "failed to deserialize github releases");
        StatusCode::INTERNAL_SERVER_ERROR
    })
}

fn is_prerelease_version(version: &str) -> bool {
    version.contains('-')
}

fn semver_is_newer(candidate: &str, current: &str) -> bool {
    parse_semver(candidate)
        .zip(parse_semver(current))
        .map(|(c, cur)| c > cur)
        .unwrap_or(false)
}

fn parse_semver(v: &str) -> Option<(u32, u32, u32, u32)> {
    let v = v.trim_start_matches('v');
    let mut segments = v.splitn(2, '-');
    let base = segments.next().unwrap_or(v);
    let pre_release = segments.next();

    let parts: Vec<&str> = base.split('.').collect();
    if parts.len() != 3 {
        return None;
    }

    let pre_release_number = pre_release
        .and_then(|p| p.rsplit('.').next())
        .and_then(|n| n.parse().ok())
        .unwrap_or(u32::MAX);

    Some((
        parts[0].parse().ok()?,
        parts[1].parse().ok()?,
        parts[2].parse().ok()?,
        pre_release_number,
    ))
}
