use crate::builds::{Builds, Completion};
use crate::edge::{self, Edge};
use crate::error::ServiceError;
use crate::git::{self, SnapshotRequest};
use crate::job::BuildJob;
use crate::{artifacts, sites, token};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{header, HeaderMap, StatusCode},
    routing::{get, post, put},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::Arc;
use uuid::Uuid;

const MAX_LOG_CHUNK_BYTES: usize = 1024 * 1024;

fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
}

fn require_admin(builds: &Builds, headers: &HeaderMap) -> Result<(), ServiceError> {
    token::secret_valid(&builds.config.secret, bearer(headers))
        .then_some(())
        .ok_or(ServiceError::Unauthorized)
}

fn require_deployment(builds: &Builds, headers: &HeaderMap, id: Uuid) -> Result<(), ServiceError> {
    token::deployment_token_valid(&builds.config.secret, id, bearer(headers))
        .then_some(())
        .ok_or(ServiceError::Unauthorized)
}

async fn detect_handler(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Json(request): Json<SnapshotRequest>,
) -> Result<Json<Value>, ServiceError> {
    require_admin(&builds, &headers)?;
    let snapshot = git::snapshot(&request).await?;
    let detection = site_builder::detect::detect(&snapshot);
    Ok(Json(json!({ "detection": detection })))
}

async fn start_build(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Json(job): Json<BuildJob>,
) -> Result<StatusCode, ServiceError> {
    require_admin(&builds, &headers)?;
    builds.start(job).await?;
    Ok(StatusCode::ACCEPTED)
}

#[derive(Debug, Default, Deserialize)]
struct PromoteRequest {
    #[serde(default)]
    env: BTreeMap<String, String>,
}

async fn promote_handler(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    body: Option<Json<PromoteRequest>>,
) -> Result<StatusCode, ServiceError> {
    require_admin(&builds, &headers)?;
    let env = body.map(|Json(request)| request.env).unwrap_or_default();
    builds.promote(id, env).await?;
    Ok(StatusCode::OK)
}

async fn reconcile_handler(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ServiceError> {
    require_admin(&builds, &headers)?;
    sites::reconcile_site(&builds, id).await?;
    Ok(StatusCode::OK)
}

async fn log_handler(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    body: String,
) -> Result<StatusCode, ServiceError> {
    require_deployment(&builds, &headers, id)?;
    builds.append_log(id, &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn artifact_handler(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    body: axum::body::Bytes,
) -> Result<StatusCode, ServiceError> {
    require_deployment(&builds, &headers, id)?;
    if builds.deployment(id).await?.status != "building" {
        return Err(ServiceError::Conflict("deployment is not building"));
    }
    artifacts::store(&builds.db, id, body.to_vec()).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn complete_handler(
    State(builds): State<Arc<Builds>>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(completion): Json<Completion>,
) -> Result<StatusCode, ServiceError> {
    require_deployment(&builds, &headers, id)?;
    builds.finish(id, completion).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn health() -> &'static str {
    "Site Builder OK"
}

pub fn control_router(builds: Arc<Builds>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/internal/detect", post(detect_handler))
        .route("/internal/builds", post(start_build))
        .route("/internal/deployments/{id}/promote", post(promote_handler))
        .route("/internal/sites/{id}/reconcile", post(reconcile_handler))
        .route(
            "/callbacks/{id}/log",
            post(log_handler).layer(DefaultBodyLimit::max(MAX_LOG_CHUNK_BYTES)),
        )
        .route(
            "/callbacks/{id}/artifact",
            put(artifact_handler).layer(DefaultBodyLimit::max(artifacts::MAX_ARCHIVE_BYTES)),
        )
        .route("/callbacks/{id}/complete", post(complete_handler))
        .with_state(builds)
}

pub fn edge_router(edge: Arc<Edge>) -> Router {
    Router::new().fallback(edge::serve).with_state(edge)
}
