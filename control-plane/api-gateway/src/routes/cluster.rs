use axum::{
    extract::{Path, State},
    http::{header::HOST, HeaderMap, StatusCode},
    response::Json,
    routing::{delete, post},
    Router,
};
use entity::entities::{agents, workloads};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use super::masters::{self, ROLE_MASTER};
use crate::{auth::rbac::CanManageSystem, setup::ca_fingerprint, AppState};

const DEFAULT_TTL_HOURS: i64 = 1;
const MAX_TTL_HOURS: i64 = 24 * 7;
const ROLE_WORKER: &str = "worker";
const ACTIVE_WORKLOAD_STATUSES: [&str; 3] = ["pending", "scheduled", "running"];

pub(super) type ApiError = (StatusCode, Json<Value>);

#[derive(Debug, Deserialize)]
pub struct CreateJoinTokenRequest {
    pub ttl_hours: Option<i64>,
    pub role: Option<String>,
}

fn join_role(requested: Option<String>) -> Result<String, ApiError> {
    match requested.as_deref() {
        None | Some(ROLE_WORKER) => Ok(ROLE_WORKER.to_string()),
        Some(ROLE_MASTER) => Ok(ROLE_MASTER.to_string()),
        Some(_) => Err(error(
            StatusCode::BAD_REQUEST,
            "role must be worker or master",
        )),
    }
}

pub(super) fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

pub(super) fn internal(err: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %err, "cluster request failed");
    error(StatusCode::INTERNAL_SERVER_ERROR, "internal error")
}

fn gateway_host(headers: &HeaderMap) -> Result<&str, ApiError> {
    let host = headers
        .get(HOST)
        .and_then(|value| value.to_str().ok())
        .filter(|host| !host.is_empty())
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "host header required"))?;
    if is_loopback_host(host) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "open the web UI through the node address instead of localhost to create a join string",
        ));
    }
    Ok(host)
}

fn is_loopback_host(host: &str) -> bool {
    let name = host.rsplit_once(':').map_or(host, |(name, _)| name);
    name.eq_ignore_ascii_case("localhost") || name.starts_with("127.") || name == "[::1]"
}

fn compact_fingerprint(fingerprint: &str) -> String {
    fingerprint.replace(':', "").to_ascii_lowercase()
}

async fn forward_to_registry(
    state: &AppState,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<Option<Value>, ApiError> {
    let (status, response) = state
        .service_client
        .forward_to_registry(method, path, body, None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "registry request failed");
            error(StatusCode::BAD_GATEWAY, "registry unavailable")
        })?;
    if status.is_success() {
        return Ok(response);
    }
    let status = StatusCode::from_u16(status.as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    Err((
        status,
        Json(response.unwrap_or_else(|| json!({ "error": "registry request rejected" }))),
    ))
}

pub async fn create_join_token(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<CreateJoinTokenRequest>,
) -> Result<Json<Value>, ApiError> {
    let ttl_hours = request.ttl_hours.unwrap_or(DEFAULT_TTL_HOURS);
    if !(1..=MAX_TTL_HOURS).contains(&ttl_hours) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "ttl_hours must be between 1 and 168",
        ));
    }
    let role = join_role(request.role)?;
    let host = gateway_host(&headers)?;
    let fingerprint = ca_fingerprint()
        .ok_or_else(|| error(StatusCode::SERVICE_UNAVAILABLE, "cluster CA unavailable"))?;

    let body = json!({
        "description": "cluster join token",
        "ttl_hours": ttl_hours,
        "max_uses": 1,
        "role": role,
    });
    let token = forward_to_registry(
        &state,
        reqwest::Method::POST,
        "/admin/bootstrap-tokens",
        Some(body),
    )
    .await?
    .ok_or_else(|| internal("registry returned no token"))?;
    let value = token["token"]
        .as_str()
        .ok_or_else(|| internal("registry token missing"))?;

    tracing::info!(
        user_id = %claims.user_id,
        role = %token["role"],
        ttl_hours = ttl_hours,
        "join token issued"
    );

    Ok(Json(json!({
        "join_string": format!(
            "csfx-join.{host}#{value}#{}#{role}",
            compact_fingerprint(&fingerprint)
        ),
        "role": token["role"],
        "expires_at": token["expires_at"],
    })))
}

pub async fn remove_node(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "node not found"))?;
    if !agent.cordoned {
        return Err(error(
            StatusCode::CONFLICT,
            "node must be cordoned before removal",
        ));
    }
    let active_workloads = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.eq(agent_id))
        .filter(workloads::Column::Status.is_in(ACTIVE_WORKLOAD_STATUSES))
        .count(&state.db_conn)
        .await
        .map_err(internal)?;
    if active_workloads > 0 {
        return Err(error(
            StatusCode::CONFLICT,
            "node still has active workloads",
        ));
    }

    let etcd_member = masters::plan_removal(&agent).await?;

    forward_to_registry(
        &state,
        reqwest::Method::DELETE,
        &format!("/admin/agents/{agent_id}"),
        None,
    )
    .await?;

    if let Some(member_id) = etcd_member {
        masters::finish_removal(member_id, &agent).await?;
    }

    tracing::warn!(user_id = %claims.user_id, agent_id = %agent_id, "node removed");
    Ok(StatusCode::NO_CONTENT)
}

pub fn cluster_routes() -> Router<AppState> {
    Router::new()
        .route("/cluster/join-tokens", post(create_join_token))
        .route("/cluster/nodes/{id}", delete(remove_node))
}
