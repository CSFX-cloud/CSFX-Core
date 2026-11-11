use crate::{
    openbao::OpenBaoError,
    routes::vaults::ApiError as VaultApiError,
    site_client::SiteBuilderError,
    vault_service::{self, VaultError},
    vault_store, AppState,
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use chrono::Utc;
use entity::{site_deployments, sites, SiteEnvVars, Vaults};
use sea_orm::{ActiveModelTrait, ColumnTrait, DbErr, EntityTrait, QueryFilter, Set};
use serde::Deserialize;
use serde_json::{json, Value};
use site_builder::detect::Mode;
use std::collections::BTreeMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum SiteError {
    #[error("{0}")]
    Invalid(String),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Conflict(&'static str),
    #[error("site hosting is not configured")]
    Unavailable,
    #[error(transparent)]
    Vault(#[from] VaultError),
    #[error(transparent)]
    Builder(#[from] SiteBuilderError),
    #[error(transparent)]
    Database(#[from] DbErr),
}

impl IntoResponse for SiteError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            Self::Vault(error) => return VaultApiError::from(error).into_response(),
            Self::Invalid(message) => (StatusCode::BAD_REQUEST, message),
            Self::NotFound => (StatusCode::NOT_FOUND, "not found".to_string()),
            Self::Conflict(message) => (StatusCode::CONFLICT, message.to_string()),
            Self::Unavailable => (StatusCode::SERVICE_UNAVAILABLE, self.to_string()),
            Self::Builder(SiteBuilderError::Rejected(message)) => {
                (StatusCode::BAD_GATEWAY, message)
            }
            Self::Builder(error) => (StatusCode::SERVICE_UNAVAILABLE, error.to_string()),
            Self::Database(error) => {
                tracing::error!(error = %error, "site database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "database error".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Detected {
    pub framework: Option<String>,
    pub mode: Mode,
    pub install_command: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub start_command: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Deserialize)]
pub struct Overrides {
    pub mode: Option<Mode>,
    pub framework: Option<String>,
    pub install_command: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub start_command: Option<String>,
    pub container_port: Option<u16>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Resolved {
    pub mode: Mode,
    pub framework: Option<String>,
    pub install_command: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub start_command: Option<String>,
    pub container_port: Option<i32>,
}

fn pick(requested: &Option<String>, detected: Option<&String>) -> Option<String> {
    requested
        .as_ref()
        .filter(|value| !value.trim().is_empty())
        .or(detected)
        .cloned()
}

pub fn resolve(overrides: &Overrides, detected: Option<&Detected>) -> Result<Resolved, SiteError> {
    let mode = overrides
        .mode
        .or(detected.map(|found| found.mode))
        .ok_or_else(|| {
            SiteError::Invalid("could not detect the project type, choose a mode".to_string())
        })?;
    let resolved = Resolved {
        mode,
        framework: pick(
            &overrides.framework,
            detected.and_then(|d| d.framework.as_ref()),
        ),
        install_command: pick(
            &overrides.install_command,
            detected.and_then(|d| d.install_command.as_ref()),
        ),
        build_command: pick(
            &overrides.build_command,
            detected.and_then(|d| d.build_command.as_ref()),
        ),
        output_dir: pick(
            &overrides.output_dir,
            detected.and_then(|d| d.output_dir.as_ref()),
        ),
        start_command: pick(
            &overrides.start_command,
            detected.and_then(|d| d.start_command.as_ref()),
        ),
        container_port: overrides
            .container_port
            .or(detected.and_then(|d| d.port))
            .map(i32::from),
    };
    check_resolved(&resolved)?;
    Ok(resolved)
}

fn check_resolved(resolved: &Resolved) -> Result<(), SiteError> {
    let invalid = |message: &str| Err(SiteError::Invalid(message.to_string()));
    match resolved.mode {
        Mode::Static if resolved.output_dir.is_none() => invalid("an output directory is required"),
        Mode::Container if resolved.container_port.is_none() => {
            invalid("a container port is required")
        }
        _ => Ok(()),
    }
}

pub fn scope_applies(scope: &str, environment: &str) -> bool {
    scope == "all" || scope == environment
}

#[derive(Debug, PartialEq, Eq)]
pub struct HookEvent {
    pub branch: String,
    pub sha: String,
    pub message: String,
    pub pull_request: Option<i32>,
}

const EMPTY_SHA: &str = "0000000000000000000000000000000000000000";
const PULL_REQUEST_ACTIONS: [&str; 3] = ["opened", "reopened", "synchronize"];

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or_default().to_string()
}

fn parse_push(payload: &Value) -> Option<HookEvent> {
    let branch = payload["ref"]
        .as_str()?
        .strip_prefix("refs/heads/")?
        .to_string();
    let sha = payload["after"]
        .as_str()
        .filter(|sha| *sha != EMPTY_SHA)?
        .to_string();
    let message = payload["head_commit"]["message"]
        .as_str()
        .unwrap_or_default();
    Some(HookEvent {
        branch,
        sha,
        message: first_line(message),
        pull_request: None,
    })
}

fn parse_pull_request(payload: &Value) -> Option<HookEvent> {
    let action = payload["action"].as_str()?;
    if !PULL_REQUEST_ACTIONS.contains(&action) {
        return None;
    }
    let request = &payload["pull_request"];
    let head_repo = request["head"]["repo"]["full_name"].as_str()?;
    if request["base"]["repo"]["full_name"].as_str()? != head_repo {
        return None;
    }
    Some(HookEvent {
        branch: request["head"]["ref"].as_str()?.to_string(),
        sha: request["head"]["sha"].as_str()?.to_string(),
        message: first_line(request["title"].as_str().unwrap_or_default()),
        pull_request: i32::try_from(request["number"].as_i64()?).ok(),
    })
}

pub fn parse_hook(event: &str, payload: &Value) -> Option<HookEvent> {
    match event {
        "push" => parse_push(payload),
        "pull_request" => parse_pull_request(payload),
        _ => None,
    }
}

pub struct SecretSource {
    pub resource_group_id: Uuid,
    pub vault_id: Option<Uuid>,
    pub actor: String,
}

impl From<&sites::Model> for SecretSource {
    fn from(site: &sites::Model) -> Self {
        Self {
            resource_group_id: site.resource_group_id,
            vault_id: site.vault_id,
            actor: format!("site:{}", site.id),
        }
    }
}

async fn source_vault(
    state: &AppState,
    source: &SecretSource,
) -> Result<entity::vaults::Model, SiteError> {
    let vault_id = source
        .vault_id
        .ok_or_else(|| SiteError::Invalid("the site has no key vault".to_string()))?;
    let vault = Vaults::find_by_id(vault_id)
        .filter(entity::vaults::Column::ResourceGroupId.eq(source.resource_group_id))
        .one(&state.db_conn)
        .await?
        .ok_or(SiteError::NotFound)?;
    vault_service::ensure_unsealed(&vault)?;
    Ok(vault)
}

pub async fn fetch_secret(
    state: &AppState,
    source: &SecretSource,
    name: &str,
) -> Result<String, SiteError> {
    let vault = source_vault(state, source).await?;
    let bao = vault_service::openbao(state)?;
    let outcome = vault_store::read_secret(bao, vault_service::scope_of(&vault), name, None).await;
    let result = if outcome.is_ok() { "ok" } else { "error" };
    vault_service::record_audit(
        &state.db_conn,
        vault.id,
        &source.actor,
        "read_secret",
        name,
        result,
    )
    .await?;
    outcome
        .map(|secret| secret.value)
        .map_err(|error| match error {
            OpenBaoError::Rejected { status: 404, .. } => {
                SiteError::Invalid(format!("secret {name} does not exist"))
            }
            other => SiteError::Vault(VaultError::Store(other)),
        })
}

pub async fn git_token(state: &AppState, site: &sites::Model) -> Result<Option<String>, SiteError> {
    match &site.git_token_secret {
        Some(name) => fetch_secret(state, &site.into(), name).await.map(Some),
        None => Ok(None),
    }
}

pub fn secret_matches(expected: &str, presented: &str) -> bool {
    const PROBE: &[u8] = b"csfx-site-hook";
    let tag = ring::hmac::sign(
        &ring::hmac::Key::new(ring::hmac::HMAC_SHA256, presented.as_bytes()),
        PROBE,
    );
    let key = ring::hmac::Key::new(ring::hmac::HMAC_SHA256, expected.as_bytes());
    ring::hmac::verify(&key, PROBE, tag.as_ref()).is_ok()
}

pub async fn environment(
    state: &AppState,
    site: &sites::Model,
    target: &str,
) -> Result<BTreeMap<String, String>, SiteError> {
    let rows = SiteEnvVars::find()
        .filter(entity::site_env_vars::Column::SiteId.eq(site.id))
        .all(&state.db_conn)
        .await?;
    let mut values = BTreeMap::new();
    for row in rows
        .into_iter()
        .filter(|row| scope_applies(&row.scope, target))
    {
        let value = match (row.value, row.vault_secret) {
            (Some(value), _) => value,
            (None, Some(secret)) => fetch_secret(state, &site.into(), &secret).await?,
            (None, None) => continue,
        };
        values.insert(row.name, value);
    }
    Ok(values)
}

pub async fn build_job(
    state: &AppState,
    site: &sites::Model,
    deployment: &site_deployments::Model,
) -> Result<Value, SiteError> {
    let env = environment(state, site, &deployment.environment).await?;
    Ok(json!({
        "deployment_id": deployment.id,
        "site_id": site.id,
        "resource_group_id": site.resource_group_id,
        "site_name": site.name,
        "mode": site.mode,
        "repo_url": site.repo_url,
        "branch": deployment.branch,
        "root_dir": site.root_dir,
        "framework": site.framework,
        "install_command": site.install_command,
        "build_command": site.build_command,
        "output_dir": site.output_dir,
        "start_command": site.start_command,
        "container_port": site.container_port,
        "git_token": git_token(state, site).await?,
        "env": env,
    }))
}

pub struct NewDeployment {
    pub branch: String,
    pub sha: String,
    pub message: String,
    pub environment: &'static str,
    pub pull_request: Option<i32>,
}

pub async fn queue(
    state: &AppState,
    site: &sites::Model,
    request: NewDeployment,
) -> Result<site_deployments::Model, SiteError> {
    let deployment = site_deployments::ActiveModel {
        id: Set(Uuid::new_v4()),
        site_id: Set(site.id),
        commit_sha: Set(request.sha),
        commit_message: Set(request.message),
        branch: Set(request.branch),
        pull_request: Set(request.pull_request),
        environment: Set(request.environment.to_string()),
        status: Set("queued".to_string()),
        build_log: Set(String::new()),
        created_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(&state.db_conn)
    .await?;
    dispatch(state, site, deployment).await
}

async fn dispatch(
    state: &AppState,
    site: &sites::Model,
    deployment: site_deployments::Model,
) -> Result<site_deployments::Model, SiteError> {
    let builder = state.site_builder.as_ref().ok_or(SiteError::Unavailable)?;
    let outcome = match build_job(state, site, &deployment).await {
        Ok(job) => builder
            .post("/internal/builds", &job)
            .await
            .map_err(SiteError::from),
        Err(error) => Err(error),
    };
    match outcome {
        Ok(_) => Ok(deployment),
        Err(error) => {
            mark_failed(state, deployment, &error.to_string()).await?;
            Err(error)
        }
    }
}

async fn mark_failed(
    state: &AppState,
    deployment: site_deployments::Model,
    message: &str,
) -> Result<(), DbErr> {
    let mut active: site_deployments::ActiveModel = deployment.into();
    active.status = Set("failed".to_string());
    active.build_log = Set(format!("[ERROR] {message}\n"));
    active.finished_at = Set(Some(Utc::now().naive_utc()));
    active.update(&state.db_conn).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detected(mode: Mode) -> Detected {
        Detected {
            framework: Some("vite".to_string()),
            mode,
            install_command: Some("npm ci".to_string()),
            build_command: Some("npm run build".to_string()),
            output_dir: Some("dist".to_string()),
            start_command: None,
            port: None,
        }
    }

    fn overrides() -> Overrides {
        Overrides {
            mode: None,
            framework: None,
            install_command: None,
            build_command: Some("  ".to_string()),
            output_dir: Some("out".to_string()),
            start_command: None,
            container_port: None,
        }
    }

    #[test]
    fn explicit_values_win_and_blank_values_fall_back_to_detection() {
        let resolved = resolve(&overrides(), Some(&detected(Mode::Static))).unwrap();
        assert_eq!(resolved.output_dir.as_deref(), Some("out"));
        assert_eq!(resolved.build_command.as_deref(), Some("npm run build"));
        assert_eq!(resolved.mode, Mode::Static);
    }

    #[test]
    fn undetectable_projects_need_an_explicit_mode() {
        assert!(resolve(&overrides(), None).is_err());
        let mut chosen = overrides();
        chosen.mode = Some(Mode::Static);
        assert!(resolve(&chosen, None).is_ok());
    }

    #[test]
    fn container_mode_needs_a_port() {
        let mut container = overrides();
        container.mode = Some(Mode::Container);
        assert!(resolve(&container, None).is_err());
        container.container_port = Some(3000);
        assert_eq!(
            resolve(&container, None).unwrap().container_port,
            Some(3000)
        );
    }

    #[test]
    fn env_scopes_match_the_target_environment() {
        assert!(scope_applies("all", "preview"));
        assert!(scope_applies("production", "production"));
        assert!(!scope_applies("production", "preview"));
    }

    #[test]
    fn push_events_on_branches_are_parsed() {
        let payload = json!({
            "ref": "refs/heads/main",
            "after": "abc123",
            "head_commit": { "message": "feat: x\n\nbody" }
        });
        let event = parse_hook("push", &payload).unwrap();
        assert_eq!(event.branch, "main");
        assert_eq!(event.message, "feat: x");
        assert!(parse_hook("push", &json!({ "ref": "refs/tags/v1", "after": "a" })).is_none());
        assert!(parse_hook(
            "push",
            &json!({ "ref": "refs/heads/m", "after": EMPTY_SHA })
        )
        .is_none());
    }

    #[test]
    fn pull_requests_from_forks_and_closed_actions_are_ignored() {
        let mut payload = json!({
            "action": "opened",
            "pull_request": {
                "number": 48, "title": "Coupons",
                "head": { "ref": "feature/coupons", "sha": "def", "repo": { "full_name": "org/shop" } },
                "base": { "repo": { "full_name": "org/shop" } }
            }
        });
        let event = parse_hook("pull_request", &payload).unwrap();
        assert_eq!(event.pull_request, Some(48));
        assert_eq!(event.branch, "feature/coupons");
        payload["pull_request"]["head"]["repo"]["full_name"] = json!("fork/shop");
        assert!(parse_hook("pull_request", &payload).is_none());
        payload["action"] = json!("closed");
        assert!(parse_hook("pull_request", &payload).is_none());
        assert!(parse_hook("issues", &payload).is_none());
    }
}
