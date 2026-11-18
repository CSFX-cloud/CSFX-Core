use axum::{
    body::Bytes,
    extract::{Path, RawQuery, State},
    http::{HeaderMap, Method},
    response::Response,
    routing::{delete, get, post},
    Router,
};
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageSystem, CanViewAgents},
    routes::proxy::{proxy, with_query, ProxyError},
    service_client::Service,
    AppState,
};

async fn failover(
    state: &AppState,
    method: Method,
    path: &str,
    headers: &HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    proxy(
        state,
        Service::FailoverController,
        method,
        path,
        headers,
        body,
    )
    .await
}

pub async fn list_events(
    CanViewAgents(_claims): CanViewAgents,
    State(state): State<AppState>,
    headers: HeaderMap,
    RawQuery(query): RawQuery,
) -> Result<Response, ProxyError> {
    let path = with_query("/events", query);
    failover(&state, Method::GET, &path, &headers, Bytes::new()).await
}

pub async fn resolve_event(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(event_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/events/{event_id}/resolve");
    failover(&state, Method::POST, &path, &headers, Bytes::new()).await
}

pub async fn set_maintenance(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ProxyError> {
    let path = format!("/agents/{agent_id}/maintenance");
    failover(&state, Method::POST, &path, &headers, body).await
}

pub async fn clear_maintenance(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ProxyError> {
    let path = format!("/agents/{agent_id}/maintenance");
    failover(&state, Method::DELETE, &path, &headers, Bytes::new()).await
}

pub fn events_routes() -> Router<AppState> {
    Router::new()
        .route("/events", get(list_events))
        .route("/events/{event_id}/resolve", post(resolve_event))
        .route("/agents/{agent_id}/maintenance", post(set_maintenance))
        .route("/agents/{agent_id}/maintenance", delete(clear_maintenance))
}
