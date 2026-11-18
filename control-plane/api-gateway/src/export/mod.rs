pub mod crypto;
pub mod manifest;
pub mod pipeline;
pub mod s3;
pub mod sink;

use axum::body::Bytes;
use futures_util::Stream;
use sea_orm::DbErr;
use std::pin::Pin;
use thiserror::Error;

use crate::oidc::secrets::SecretError;
use crypto::CryptoError;
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
