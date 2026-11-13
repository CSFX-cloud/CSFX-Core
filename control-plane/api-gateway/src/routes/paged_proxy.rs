use axum::{
    body::Body,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
};
use serde_json::json;
use shared::pagination::cursor_headers;

use crate::{service_client::ListTarget, AppState};

pub fn with_query(path: &str, query: Option<String>) -> String {
    match query {
        Some(query) => format!("{}?{}", path, query),
        None => path.to_string(),
    }
}

pub async fn proxy_list(
    state: &AppState,
    target: ListTarget,
    path_and_query: &str,
    headers: &HeaderMap,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let forwarded = headers
        .iter()
        .filter_map(|(k, v)| v.to_str().ok().map(|val| (k.to_string(), val.to_string())))
        .collect();
    let reply = state
        .service_client
        .forward_list(target, path_and_query, forwarded)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, path = path_and_query, "list proxy failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": "Upstream service unavailable", "details": e.to_string() })),
            )
        })?;
    let status =
        StatusCode::from_u16(reply.status.as_u16()).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
    let cursor = cursor_headers(reply.next_cursor);
    Ok(match reply.body {
        Some(body) => (status, cursor, Json(body)).into_response(),
        None => (status, cursor, Body::empty()).into_response(),
    })
}
