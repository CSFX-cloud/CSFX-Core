use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use shared::pagination::{cursor_headers, split_page, PageParams};
use uuid::Uuid;

use crate::db::events::{EventCursor, EventFilter};
use crate::server::AppState;

#[derive(Debug, Deserialize)]
pub struct ListEventsQuery {
    status: Option<String>,
    agent_id: Option<Uuid>,
}

const DEFAULT_EVENT_LIMIT: u64 = 100;

pub async fn list_events(
    State(state): State<AppState>,
    Query(query): Query<ListEventsQuery>,
    Query(page): Query<PageParams>,
) -> impl IntoResponse {
    let after = match page.cursor.as_deref().map(EventCursor::decode) {
        Some(None) => return invalid_cursor(),
        Some(Some(cursor)) => Some(cursor),
        None => None,
    };
    let filter = EventFilter {
        status: query.status,
        agent_id: query.agent_id,
        after,
        limit: page.fetch_limit().unwrap_or(DEFAULT_EVENT_LIMIT),
    };
    match crate::db::events::list(&state.db, filter).await {
        Ok(events) => {
            let (events, next) = split_page(events, &page, |e| EventCursor::of(e).encode());
            (StatusCode::OK, cursor_headers(next), Json(events)).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

fn invalid_cursor() -> axum::response::Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({ "error": "invalid cursor" })),
    )
        .into_response()
}

pub async fn resolve_event(
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
) -> impl IntoResponse {
    match crate::db::events::resolve(&state.db, event_id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
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
