use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post, put},
    Router,
};
use entity::entities::{agent_metrics, agents, resource_groups, volumes, workloads};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use shared::pagination::{cursor_headers, keyset_by_id, split_page, PageParams};
use shared::workload_status::ACTIVE_STATUSES;
use uuid::Uuid;

use crate::auth::agent::AgentApiKey;
use crate::auth::rbac::{CanManageSystem, CanViewAgents};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentResponse {
    pub id: Uuid,
    pub name: String,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub agent_version: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub status: String,
    pub last_heartbeat: Option<String>,
    pub registered_at: String,
    pub cordoned: bool,
    pub maintenance_until: Option<String>,
    pub role: String,
    pub cpu_overcommit_ratio: f64,
    pub memory_overcommit_ratio: f64,
    pub disk_overcommit_ratio: f64,
    pub ssh_host_key: Option<String>,
}

impl From<agents::Model> for AgentResponse {
    fn from(model: agents::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            hostname: model.hostname,
            ip_address: model.ip_address,
            agent_version: model.agent_version,
            os_type: model.os_type,
            os_version: model.os_version,
            architecture: model.architecture,
            status: model.status,
            last_heartbeat: model.last_heartbeat.map(|dt| dt.to_string()),
            registered_at: model.registered_at.to_string(),
            cordoned: model.cordoned,
            maintenance_until: model.maintenance_until.map(|dt| dt.and_utc().to_rfc3339()),
            role: model.role,
            cpu_overcommit_ratio: model.cpu_overcommit_ratio,
            memory_overcommit_ratio: model.memory_overcommit_ratio,
            disk_overcommit_ratio: model.disk_overcommit_ratio,
            ssh_host_key: model.ssh_host_key,
        }
    }
}

fn is_container_id(hostname: &str) -> bool {
    let h = hostname.trim();
    h.len() == 12 && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// List all agents
pub async fn list_agents(
    State(state): State<AppState>,
    _perm: CanViewAgents,
    Query(page): Query<PageParams>,
) -> Result<impl IntoResponse, StatusCode> {
    let after = page.cursor_id().map_err(|_| StatusCode::BAD_REQUEST)?;
    let query = match page.limit() {
        Some(_) => keyset_by_id(agents::Entity::find(), agents::Column::Id, after, &page),
        None => agents::Entity::find().order_by_desc(agents::Column::RegisteredAt),
    };
    let agents = query.all(&state.db_conn).await.map_err(|e| {
        tracing::error!("Failed to fetch agents: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let (agents, next) = split_page(agents, &page, |a| a.id.to_string());
    let response: Vec<AgentResponse> = agents
        .into_iter()
        .filter(|a| !is_container_id(&a.hostname))
        .map(Into::into)
        .collect();
    Ok((cursor_headers(next), Json(response)))
}

/// Get agent by ID
pub async fn get_agent(
    State(state): State<AppState>,
    _perm: CanViewAgents,
    axum::extract::Path(agent_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(AgentResponse::from(agent)))
}

/// Get latest metrics for an agent
pub async fn get_agent_metrics(
    State(state): State<AppState>,
    _perm: CanViewAgents,
    axum::extract::Path(agent_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let metrics: Vec<agent_metrics::Model> = agent_metrics::Entity::find()
        .filter(agent_metrics::Column::AgentId.eq(agent_id))
        .order_by_desc(agent_metrics::Column::Timestamp)
        .limit(100)
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch metrics: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(metrics))
}

pub async fn get_agent_metrics_latest(
    State(state): State<AppState>,
    _perm: CanViewAgents,
    axum::extract::Path(agent_id): axum::extract::Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let metric = agent_metrics::Entity::find()
        .filter(agent_metrics::Column::AgentId.eq(agent_id))
        .order_by_desc(agent_metrics::Column::Timestamp)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch latest agent metrics");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(metric))
}

#[derive(Debug, Serialize)]
pub struct AssignedWorkloadResponse {
    #[serde(flatten)]
    pub workload: workloads::Model,
    pub resource_group_cidr: Option<String>,
}

pub async fn get_self_workloads(
    agent: AgentApiKey,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let rows = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.eq(agent.agent_id))
        .filter(workloads::Column::DesiredState.ne("stopped"))
        .all(&state.agent_db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch workloads for agent");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut response = Vec::with_capacity(rows.len());
    for workload in rows {
        let resource_group_cidr = match workload.resource_group_id {
            Some(rg_id) => resource_groups::Entity::find_by_id(rg_id)
                .one(&state.agent_db)
                .await
                .map_err(|e| {
                    tracing::error!(error = %e, "failed to fetch resource group for workload");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?
                .map(|rg| rg.internal_cidr),
            None => None,
        };

        response.push(AssignedWorkloadResponse {
            workload,
            resource_group_cidr,
        });
    }

    Ok(Json(response))
}

pub async fn push_self_workload_stats(
    agent: AgentApiKey,
    State(state): State<AppState>,
    body: String,
) -> Result<impl IntoResponse, StatusCode> {
    let mut body_json: serde_json::Value =
        serde_json::from_str(&body).map_err(|_| StatusCode::BAD_REQUEST)?;
    let payload = body_json.as_object_mut().ok_or(StatusCode::BAD_REQUEST)?;
    payload.insert("agent_id".to_string(), serde_json::json!(agent.agent_id));
    match state
        .service_client
        .forward_to_scheduler(
            reqwest::Method::POST,
            "/internal/workloads/stats",
            Some(body_json),
            None,
        )
        .await
    {
        Ok((status, _)) => Ok(StatusCode::from_u16(status.as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response()),
        Err(e) => {
            tracing::error!(error = %e, "failed to forward workload stats to scheduler");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

pub async fn ack_self_workload_restart(
    agent: AgentApiKey,
    State(state): State<AppState>,
    Path(workload_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let path = format!("/internal/workloads/{}/restart-ack", workload_id);
    let body = serde_json::json!({ "agent_id": agent.agent_id });
    match state
        .service_client
        .forward_to_scheduler(reqwest::Method::POST, &path, Some(body), None)
        .await
    {
        Ok((status, _)) => Ok(StatusCode::from_u16(status.as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response()),
        Err(e) => {
            tracing::error!(error = %e, "failed to forward restart ack to scheduler");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

pub async fn get_self_volumes(
    agent: AgentApiKey,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, StatusCode> {
    let rows = volumes::Entity::find()
        .filter(volumes::Column::AttachedToAgent.eq(agent.agent_id))
        .all(&state.agent_db)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch volumes for agent");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(rows))
}

pub async fn drain_agent(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch agent for drain");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: agents::ActiveModel = agent.into();
    active.cordoned = ActiveValue::Set(true);
    active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to cordon agent");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    let workload_ids: Vec<Uuid> = workloads::Entity::find()
        .filter(workloads::Column::AssignedAgentId.eq(agent_id))
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list agent workloads for drain");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .into_iter()
        .map(|w| w.id)
        .collect();

    tracing::warn!(
        user_id = %claims.user_id,
        agent_id = %agent_id,
        workload_count = workload_ids.len(),
        "node drain requested"
    );

    let path = format!("/internal/agents/{}/reschedule", agent_id);
    let body = serde_json::json!({ "workload_ids": workload_ids });

    match state
        .service_client
        .forward_to_scheduler(reqwest::Method::POST, &path, Some(body), None)
        .await
    {
        Ok((status, _)) => Ok(StatusCode::from_u16(status.as_u16())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR)
            .into_response()),
        Err(e) => {
            tracing::error!(error = %e, "failed to forward drain reschedule to scheduler");
            Err(StatusCode::BAD_GATEWAY)
        }
    }
}

async fn set_cordoned(
    state: &AppState,
    agent_id: Uuid,
    cordoned: bool,
) -> Result<StatusCode, StatusCode> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, cordoned = cordoned, "failed to fetch agent for cordon change");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: agents::ActiveModel = agent.into();
    active.cordoned = ActiveValue::Set(cordoned);
    active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, cordoned = cordoned, "failed to change agent cordon state");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::NO_CONTENT)
}

pub async fn cordon_agent(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    set_cordoned(&state, agent_id, true).await
}

pub async fn uncordon_agent(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    set_cordoned(&state, agent_id, false).await
}

const MAX_CPU_OVERCOMMIT_RATIO: f64 = 16.0;
const MAX_MEMORY_OVERCOMMIT_RATIO: f64 = 4.0;
const MAX_DISK_OVERCOMMIT_RATIO: f64 = 4.0;

#[derive(Debug, Deserialize)]
pub struct OvercommitRequest {
    cpu_ratio: Option<f64>,
    memory_ratio: Option<f64>,
    disk_ratio: Option<f64>,
}

fn is_valid_ratio(ratio: Option<f64>, max: f64) -> bool {
    ratio.is_none_or(|value| value.is_finite() && (1.0..=max).contains(&value))
}

pub async fn set_agent_overcommit(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
    Json(req): Json<OvercommitRequest>,
) -> Result<impl IntoResponse, StatusCode> {
    if !is_valid_ratio(req.cpu_ratio, MAX_CPU_OVERCOMMIT_RATIO)
        || !is_valid_ratio(req.memory_ratio, MAX_MEMORY_OVERCOMMIT_RATIO)
        || !is_valid_ratio(req.disk_ratio, MAX_DISK_OVERCOMMIT_RATIO)
    {
        return Err(StatusCode::BAD_REQUEST);
    }

    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch agent for overcommit change");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: agents::ActiveModel = agent.into();
    if let Some(ratio) = req.cpu_ratio {
        active.cpu_overcommit_ratio = ActiveValue::Set(ratio);
    }
    if let Some(ratio) = req.memory_ratio {
        active.memory_overcommit_ratio = ActiveValue::Set(ratio);
    }
    if let Some(ratio) = req.disk_ratio {
        active.disk_overcommit_ratio = ActiveValue::Set(ratio);
    }
    active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to change agent overcommit ratios");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    tracing::info!(
        user_id = %claims.user_id,
        agent_id = %agent_id,
        "agent overcommit ratios changed"
    );

    Ok(StatusCode::NO_CONTENT)
}

pub fn agents_routes() -> Router<AppState> {
    Router::new()
        .route("/agents/{id}/overcommit", put(set_agent_overcommit))
        .route("/agents", get(list_agents))
        .route("/agents/{id}", get(get_agent))
        .route("/agents/{id}/metrics", get(get_agent_metrics))
        .route("/agents/{id}/metrics/latest", get(get_agent_metrics_latest))
        .route("/agents/{id}/cordon", post(cordon_agent))
        .route("/agents/{id}/drain", post(drain_agent))
        .route("/agents/{id}/uncordon", post(uncordon_agent))
}

pub fn agents_unmetered_routes() -> Router<AppState> {
    Router::new()
        .route("/agents/self/workloads", get(get_self_workloads))
        .route(
            "/agents/self/workloads/{id}/restart-ack",
            post(ack_self_workload_restart),
        )
        .route(
            "/agents/self/workloads/stats",
            post(push_self_workload_stats),
        )
        .route("/agents/self/volumes", get(get_self_volumes))
}
