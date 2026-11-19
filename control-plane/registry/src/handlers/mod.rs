use axum::{http::StatusCode, Json};

use crate::models::agent::ErrorResponse;

pub mod admin;
pub mod agent;
pub mod pki;

pub type ApiError = (StatusCode, Json<ErrorResponse>);

pub fn api_error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}
