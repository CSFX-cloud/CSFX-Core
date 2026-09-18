use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    status: Option<String>,
    agent_id: Option<Uuid>,
}

pub async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<ListEventsQuery>,
) -> impl IntoResponse {
    match crate::db::events::get_recent(&state.db, 100).await {
        Ok(events) => {
            let filtered: Vec<_> = events
                .into_iter()
                .filter(|e| query.status.as_deref().is_none_or(|s| e.status == s))
                .filter(|e| query.agent_id.is_none_or(|id| e.agent_id == Some(id)))
                .collect();
            (StatusCode::OK, Json(serde_json::json!(filtered))).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
pub struct SetMaintenanceRequest {
    minutes: i64,
}

pub async fn set_maintenance(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Json(body): Json<SetMaintenanceRequest>,
) -> impl IntoResponse {
    let until = chrono::Utc::now().naive_utc() + chrono::Duration::minutes(body.minutes.max(1));

    match crate::db::agents::set_maintenance(&state.db, agent_id, Some(until)).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "maintenance_until": until })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

pub async fn clear_maintenance(
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> impl IntoResponse {
    match crate::db::agents::set_maintenance(&state.db, agent_id, None).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}
