use thiserror::Error;

#[derive(Error, Debug)]
pub enum EtcdError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("etcd client error: {0}")]
    Client(#[from] etcd_client::Error),

    #[error("Leader election failed: {0}")]
    LeaderElection(String),

    #[error("State operation failed: {0}")]
    StateOperation(String),

    #[error("Lock acquisition failed: {0}")]
    LockFailed(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("No leader available")]
    NoLeader,

    #[error("Lease expired")]
    LeaseExpired,

    #[error("Watch error: {0}")]
    Watch(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

pub type Result<T> = std::result::Result<T, EtcdError>;
