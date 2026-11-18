use axum::{
    body::Bytes,
    http::{HeaderMap, Method, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;

use crate::{service_client::Service, AppState};

pub type ProxyError = (StatusCode, Json<serde_json::Value>);

pub fn with_query(path: &str, query: Option<String>) -> String {
    match query {
        Some(query) => format!("{}?{}", path, query),
        None => path.to_string(),
    }
}

pub async fn proxy(
    state: &AppState,
    service: Service,
    method: Method,
    path: &str,
    headers: &HeaderMap,
    body: impl Into<Bytes>,
) -> Result<Response, ProxyError> {
    match state
        .service_client
        .forward(service, method, path, headers, body)
        .await
    {
        Ok(upstream) => Ok(upstream.into_response()),
        Err(error) => {
            tracing::error!(service = ?service, path = path, error = %error, "upstream request failed");
            Err((
                StatusCode::BAD_GATEWAY,
                Json(
                    json!({ "error": "Upstream service unavailable", "details": error.to_string() }),
                ),
            ))
        }
    }
}
