use thiserror::Error;

#[derive(Debug, Error)]
pub enum CephError {
    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Ceph cluster not healthy: {0}")]
    UnhealthyCluster(String),

    #[error("Pool operation failed: {0}")]
    PoolError(String),

    #[error("RBD operation failed: {0}")]
    RbdError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Timeout waiting for cluster")]
    Timeout,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("UTF-8 error: {0}")]
    Utf8Error(#[from] std::string::FromUtf8Error),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

pub type Result<T> = std::result::Result<T, CephError>;
