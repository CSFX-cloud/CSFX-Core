use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};

use crate::server::AppState;

pub async fn list_events(State(state): State<AppState>) -> impl IntoResponse {
    match crate::db::events::get_recent(&state.db, 100).await {
        Ok(events) => (StatusCode::OK, Json(serde_json::json!(events))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
