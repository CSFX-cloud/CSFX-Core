use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, HeaderMap, StatusCode},
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use entity::{entities::exports, Exports};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait, QueryOrder, QuerySelect};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::{
        jwt::Claims,
        rbac::{AuthenticatedUser, CanManageSystem},
    },
    export::{
        self,
        crypto::CryptoError,
        manifest::{Scope, ScopeKind},
        sink::SinkError,
        target::{self, DestinationInput, TargetView},
        ExportError, RestoreOptions,
    },
    rbac_service::RbacService,
    AppState,
};

const LIST_LIMIT: u64 = 100;
const PASSPHRASE_HEADER: &str = "x-export-passphrase";

type ApiError = (StatusCode, Json<Value>);

fn api_error(error: ExportError) -> ApiError {
    let status = match &error {
        ExportError::NotConfigured
        | ExportError::ChecksumMismatch(_)
        | ExportError::SchemaMismatch { .. }
        | ExportError::MissingArtifact(_)
        | ExportError::UnsupportedFormat
        | ExportError::UnsupportedScope(_) => StatusCode::CONFLICT,
        ExportError::AgentRequired => StatusCode::BAD_REQUEST,
        ExportError::Upstream(_) | ExportError::PartialRestore(_) => StatusCode::BAD_GATEWAY,
        ExportError::InvalidTarget(_)
        | ExportError::PassphraseRequired
        | ExportError::ScopeIdRequired
        | ExportError::Crypto(CryptoError::WrongKey) => StatusCode::BAD_REQUEST,
        ExportError::TargetNotFound(_) | ExportError::ResourceNotFound(_) => StatusCode::NOT_FOUND,
        ExportError::Sink(SinkError::Io(io)) if io.kind() == std::io::ErrorKind::NotFound => {
            StatusCode::NOT_FOUND
        }
        ExportError::Sink(SinkError::Remote(_) | SinkError::Http(_)) => StatusCode::BAD_GATEWAY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    if status == StatusCode::INTERNAL_SERVER_ERROR {
        tracing::error!(error = %error, "export request failed");
        return (status, Json(json!({ "error": "internal error" })));
    }
    (status, Json(json!({ "error": error.to_string() })))
}

fn database_error(error: sea_orm::DbErr) -> ApiError {
    api_error(ExportError::Db(error))
}

fn forbidden() -> ApiError {
    (
        StatusCode::FORBIDDEN,
        Json(json!({ "error": "permission denied" })),
    )
}

fn required_permission(kind: ScopeKind) -> (&'static str, &'static str) {
    match kind {
        ScopeKind::Config | ScopeKind::Full => ("system", "manage"),
        ScopeKind::ResourceGroup => ("resource_groups", "manage"),
        ScopeKind::Workload => ("workloads", "manage"),
        ScopeKind::Volume => ("volumes", "manage"),
        ScopeKind::Bucket => ("buckets", "manage"),
    }
}

async fn permitted(
    state: &AppState,
    claims: &Claims,
    resource: &str,
    action: &str,
) -> Result<bool, ApiError> {
    let org_id = state.default_org_id.ok_or_else(|| {
        api_error(ExportError::Db(sea_orm::DbErr::Custom(
            "default organization missing".to_string(),
        )))
    })?;
    RbacService::new(state.db_conn.clone())
        .has_permission(claims.user_id, org_id, resource, action)
        .await
        .map_err(|error| api_error(ExportError::Db(sea_orm::DbErr::Custom(error.to_string()))))
}

#[derive(Debug, Deserialize)]
pub struct PutTargetRequest {
    #[serde(flatten)]
    pub destination: DestinationInput,
    pub passphrase: Option<String>,
    #[serde(default)]
    pub default: bool,
}

#[derive(Debug, Deserialize)]
pub struct CreateExportRequest {
    pub scope: Scope,
    pub target: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RestoreRequest {
    pub target: Option<String>,
    pub passphrase: Option<String>,
    pub agent_id: Option<Uuid>,
}

async fn list_targets(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
) -> Result<Json<Vec<TargetView>>, ApiError> {
    let views = target::list(&state.db_conn).await.map_err(api_error)?;
    Ok(Json(views))
}

async fn put_target(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(request): Json<PutTargetRequest>,
) -> Result<Json<TargetView>, ApiError> {
    let view = target::put(
        &state.db_conn,
        &name,
        request.destination,
        request.passphrase,
        request.default,
    )
    .await
    .map_err(api_error)?;
    Ok(Json(view))
}

async fn delete_target(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    target::remove(&state.db_conn, &name)
        .await
        .map_err(api_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn test_target(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    export::probe_target(&state.db_conn, &name)
        .await
        .map_err(api_error)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn create_export(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(request): Json<CreateExportRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let (resource, action) = required_permission(request.scope.kind);
    if !permitted(&state, &claims, resource, action).await? {
        return Err(forbidden());
    }
    export::validate_scope(&state.db_conn, &request.scope)
        .await
        .map_err(api_error)?;
    target::load(&state.db_conn, request.target.as_deref())
        .await
        .map_err(api_error)?;
    let id = Uuid::new_v4();
    exports::ActiveModel {
        id: Set(id),
        scope_kind: Set(request.scope.kind.as_str().to_string()),
        scope_id: Set(request.scope.id),
        status: Set("running".to_string()),
        manifest: Set(None),
        error: Set(None),
        created_by: Set(Some(claims.user_id)),
        created_at: Set(chrono::Utc::now().into()),
        finished_at: Set(None),
    }
    .insert(&state.db_conn)
    .await
    .map_err(database_error)?;
    tokio::spawn(export::run_job(
        state.clone(),
        id,
        request.scope,
        request.target,
    ));
    Ok((StatusCode::ACCEPTED, Json(json!({ "id": id }))))
}

async fn list_exports(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
) -> Result<Json<Vec<exports::Model>>, ApiError> {
    let rows = Exports::find()
        .order_by_desc(exports::Column::CreatedAt)
        .limit(LIST_LIMIT)
        .all(&state.db_conn)
        .await
        .map_err(database_error)?;
    Ok(Json(rows))
}

async fn get_export(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<exports::Model>, ApiError> {
    let row = Exports::find_by_id(id)
        .one(&state.db_conn)
        .await
        .map_err(database_error)?
        .ok_or_else(|| api_error(ExportError::ResourceNotFound(format!("export {id}"))))?;
    let owner = row.created_by == Some(claims.user_id);
    if owner || permitted(&state, &claims, "system", "manage").await? {
        return Ok(Json(row));
    }
    Err(forbidden())
}

#[derive(Debug, Deserialize)]
pub struct DownloadQuery {
    pub target: Option<String>,
}

async fn download_artifact(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path((id, name)): Path<(Uuid, String)>,
    Query(query): Query<DownloadQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, ApiError> {
    let options = RestoreOptions {
        target: query.target,
        passphrase: headers
            .get(PASSPHRASE_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string),
        agent_id: None,
    };
    let stream = export::open_artifact(&state.db_conn, id, options, &name)
        .await
        .map_err(api_error)?;
    Ok((
        [(CONTENT_TYPE, "application/octet-stream")],
        Body::from_stream(stream),
    ))
}

async fn restore_export(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<RestoreRequest>,
) -> Result<Json<Value>, ApiError> {
    let options = RestoreOptions {
        target: request.target,
        passphrase: request.passphrase,
        agent_id: request.agent_id,
    };
    let outcome = export::restore(&state, id, options)
        .await
        .map_err(api_error)?;
    tracing::info!(export_id = %id, "export restored");
    Ok(Json(outcome))
}

pub fn exports_routes() -> Router<AppState> {
    Router::new()
        .route("/admin/export-targets", get(list_targets))
        .route(
            "/admin/export-targets/{name}",
            put(put_target).delete(delete_target),
        )
        .route("/admin/export-targets/{name}/test", post(test_target))
        .route("/exports", get(list_exports).post(create_export))
        .route("/exports/{id}", get(get_export))
        .route("/exports/{id}/restore", post(restore_export))
        .route("/exports/{id}/artifacts/{*name}", get(download_artifact))
}
