pub mod crypto;
pub mod engine;
pub mod manifest;
pub mod pipeline;
pub mod s3;
pub mod sink;
pub mod sources;
pub mod tables;
pub mod target;

use axum::body::Bytes;
use chrono::Utc;
use entity::{entities::exports, Buckets, Exports, ResourceGroups, Volumes, Workloads};
use futures_util::Stream;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ConnectionTrait, DbBackend, DbConn, DbErr, EntityTrait,
    Statement,
};
use serde_json::{json, Value};
use std::pin::Pin;
use thiserror::Error;
use uuid::Uuid;

use crate::{oidc::secrets::SecretError, AppState};
use crypto::CryptoError;
use engine::{Exporter, Reader};
use manifest::{Manifest, Scope, ScopeKind};
use sink::SinkError;

pub type ByteStream = Pin<Box<dyn Stream<Item = Result<Bytes, ExportError>> + Send>>;

#[derive(Debug, Error)]
pub enum ExportError {
    #[error("no export target is configured")]
    NotConfigured,
    #[error("export target not found: {0}")]
    TargetNotFound(String),
    #[error("invalid export target: {0}")]
    InvalidTarget(String),
    #[error("database error: {0}")]
    Db(#[from] DbErr),
    #[error(transparent)]
    Sink(#[from] SinkError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Secret(#[from] SecretError),
    #[error("stored json is malformed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("checksum mismatch for artifact {0}")]
    ChecksumMismatch(String),
    #[error("export schema {found} does not match installation schema {expected}")]
    SchemaMismatch { expected: String, found: String },
    #[error("export is missing artifact {0}")]
    MissingArtifact(String),
    #[error("artifact content is malformed")]
    MalformedArtifact,
    #[error("export is encrypted and no passphrase was provided")]
    PassphraseRequired,
    #[error("unsupported export format")]
    UnsupportedFormat,
    #[error("restore is not supported for scope {0}, see the backup and restore guide")]
    UnsupportedScope(&'static str),
    #[error("scope requires an id")]
    ScopeIdRequired,
    #[error("{0} not found")]
    ResourceNotFound(String),
    #[error("upstream service failed: {0}")]
    Upstream(String),
    #[error("database url is missing or unsupported")]
    InvalidDatabaseUrl,
    #[error("restore needs an agent_id because the workload has no assigned agent")]
    AgentRequired,
    #[error("{0}")]
    PartialRestore(String),
}

pub async fn schema_migration(db: &impl ConnectionTrait) -> Result<String, DbErr> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT version FROM seaql_migrations ORDER BY version DESC LIMIT 1",
        ))
        .await?
        .ok_or_else(|| DbErr::RecordNotFound("seaql_migrations".to_string()))?;
    row.try_get("", "version")
}

async fn exists(db: &DbConn, scope: &Scope, id: Uuid) -> Result<bool, DbErr> {
    Ok(match scope.kind {
        ScopeKind::ResourceGroup => ResourceGroups::find_by_id(id).one(db).await?.is_some(),
        ScopeKind::Workload => Workloads::find_by_id(id).one(db).await?.is_some(),
        ScopeKind::Volume => Volumes::find_by_id(id).one(db).await?.is_some(),
        ScopeKind::Bucket => Buckets::find_by_id(id).one(db).await?.is_some(),
        ScopeKind::Config | ScopeKind::Full => true,
    })
}

pub async fn validate_scope(db: &DbConn, scope: &Scope) -> Result<(), ExportError> {
    if !scope.kind.needs_id() {
        return Ok(());
    }
    let id = scope.id.ok_or(ExportError::ScopeIdRequired)?;
    if exists(db, scope, id).await? {
        return Ok(());
    }
    Err(ExportError::ResourceNotFound(format!(
        "{} {id}",
        scope.kind.as_str()
    )))
}

async fn collect_scope(
    state: &AppState,
    exporter: &mut Exporter,
    scope: &Scope,
) -> Result<(), ExportError> {
    if scope.kind.needs_id() && scope.id.is_none() {
        return Err(ExportError::ScopeIdRequired);
    }
    let id = scope.id.unwrap_or_default();
    match scope.kind {
        ScopeKind::Config => sources::config::export(&state.db_conn, exporter).await,
        ScopeKind::ResourceGroup => sources::resource_group::export(state, exporter, id).await,
        ScopeKind::Workload => sources::workload::export(state, exporter, id).await,
        ScopeKind::Volume => sources::volume::export(state, exporter, id).await,
        ScopeKind::Bucket => sources::bucket::export(state, exporter, id).await,
        other => Err(ExportError::UnsupportedScope(other.as_str())),
    }
}

pub async fn run_export(
    state: &AppState,
    export_id: Uuid,
    scope: Scope,
    target_name: Option<&str>,
) -> Result<Manifest, ExportError> {
    let target = target::load(&state.db_conn, target_name).await?;
    let mut exporter = Exporter::new(export_id, target)?;
    let schema = schema_migration(&state.db_conn).await?;
    collect_scope(state, &mut exporter, &scope).await?;
    exporter.finish(schema, scope).await
}

async fn ensure_schema(db: &DbConn, manifest: &Manifest) -> Result<(), ExportError> {
    let expected = schema_migration(db).await?;
    if manifest.schema_migration == expected {
        return Ok(());
    }
    Err(ExportError::SchemaMismatch {
        expected,
        found: manifest.schema_migration.clone(),
    })
}

#[derive(Default)]
pub struct RestoreOptions {
    pub target: Option<String>,
    pub passphrase: Option<String>,
    pub agent_id: Option<Uuid>,
}

pub async fn restore(
    state: &AppState,
    export_id: Uuid,
    options: RestoreOptions,
) -> Result<Value, ExportError> {
    let target = target::load(&state.db_conn, options.target.as_deref()).await?;
    let reader = Reader::open(target, export_id, options.passphrase).await?;
    ensure_schema(&state.db_conn, &reader.manifest).await?;
    let scope = &reader.manifest.scope;
    let id = scope.id.unwrap_or_default();
    match scope.kind {
        ScopeKind::Config => {
            sources::config::restore(&state.db_conn, &reader).await?;
            Ok(json!({}))
        }
        ScopeKind::Bucket => sources::bucket::restore(state, &reader, id).await,
        ScopeKind::Volume => sources::volume::restore(state, &reader, id).await,
        ScopeKind::Workload => {
            sources::workload::restore(state, &reader, id, options.agent_id).await
        }
        ScopeKind::ResourceGroup => {
            sources::resource_group::restore(state, &reader, options.agent_id).await
        }
        other => Err(ExportError::UnsupportedScope(other.as_str())),
    }
}

pub async fn open_artifact(
    db: &DbConn,
    export_id: Uuid,
    options: RestoreOptions,
    name: &str,
) -> Result<ByteStream, ExportError> {
    let target = target::load(db, options.target.as_deref()).await?;
    let reader = Reader::open(target, export_id, options.passphrase).await?;
    reader.stream(name).await
}

pub async fn probe_target(db: &DbConn, name: &str) -> Result<(), ExportError> {
    let target = target::load(db, Some(name)).await?;
    Ok(target.sink.probe().await?)
}

async fn record_result(
    db: &DbConn,
    export_id: Uuid,
    result: Result<Manifest, ExportError>,
) -> Result<(), DbErr> {
    let row = Exports::find_by_id(export_id)
        .one(db)
        .await?
        .ok_or_else(|| DbErr::RecordNotFound(export_id.to_string()))?;
    let mut active: exports::ActiveModel = row.into();
    match result {
        Ok(manifest) => {
            let value =
                serde_json::to_value(&manifest).map_err(|e| DbErr::Custom(e.to_string()))?;
            active.status = Set("completed".to_string());
            active.manifest = Set(Some(value));
        }
        Err(error) => {
            active.status = Set("failed".to_string());
            active.error = Set(Some(error.to_string()));
        }
    }
    active.finished_at = Set(Some(Utc::now().into()));
    active.update(db).await?;
    Ok(())
}

pub async fn run_job(state: AppState, export_id: Uuid, scope: Scope, target_name: Option<String>) {
    let result = run_export(&state, export_id, scope, target_name.as_deref()).await;
    match &result {
        Ok(manifest) => tracing::info!(
            export_id = %export_id,
            artifacts = manifest.artifacts.len(),
            "export completed"
        ),
        Err(error) => tracing::error!(export_id = %export_id, error = %error, "export failed"),
    }
    if let Err(error) = record_result(&state.db_conn, export_id, result).await {
        tracing::error!(export_id = %export_id, error = %error, "failed to record export result");
    }
}
