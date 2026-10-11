use thiserror::Error;

use super::secrets::SecretError;

#[derive(Debug, Error)]
pub enum OidcError {
    #[error("http error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("database error: {0}")]
    Database(#[from] sea_orm::DbErr),
    #[error("secret error: {0}")]
    Secret(#[from] SecretError),
    #[error("token rejected: {0}")]
    Token(#[from] jsonwebtoken::errors::Error),
    #[error("invalid provider response: {0}")]
    InvalidProvider(&'static str),
    #[error("invalid or expired login state")]
    InvalidState,
    #[error("access denied: {0}")]
    AccessDenied(&'static str),
    #[error("role assignment failed: {0}")]
    RoleAssignment(String),
}

impl OidcError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Http(_) | Self::InvalidProvider(_) => "provider_unavailable",
            Self::Token(_) => "token_rejected",
            Self::InvalidState => "invalid_state",
            Self::AccessDenied(reason) => reason,
            Self::Database(_) | Self::Secret(_) | Self::RoleAssignment(_) => "internal_error",
        }
    }
}
