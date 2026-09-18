use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use entity::entities::{agent_metrics, agents, resource_groups, volumes, workloads};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::agent::AgentApiKey;
use crate::auth::rbac::{CanManageSystem, CanViewAgents};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMetrics {
    pub agent_id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,

    // CPU
    pub cpu_model: Option<String>,
    pub cpu_cores: Option<u32>,
    pub cpu_threads: Option<u32>,
    pub cpu_usage_percent: Option<f32>,

    // Memory
    pub memory_total_bytes: Option<u64>,
    pub memory_used_bytes: Option<u64>,
    pub memory_usage_percent: Option<f32>,

    // Disk
    pub disk_total_bytes: Option<u64>,
    pub disk_used_bytes: Option<u64>,
    pub disk_usage_percent: Option<f32>,

    // Network
    pub network_rx_bytes: Option<u64>,
    pub network_tx_bytes: Option<u64>,

    // System
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub hostname: Option<String>,
    pub uptime_seconds: Option<u64>,
}

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
        }
    }
}

/// Receive metrics from agent
pub async fn receive_metrics(
    State(state): State<AppState>,
    Json(metrics): Json<SystemMetrics>,
) -> Result<impl IntoResponse, StatusCode> {
    // Store metrics in database
    let new_metrics = agent_metrics::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        agent_id: ActiveValue::Set(metrics.agent_id),
        timestamp: ActiveValue::Set(metrics.timestamp.naive_utc()),
        cpu_model: ActiveValue::Set(metrics.cpu_model),
        cpu_cores: ActiveValue::Set(metrics.cpu_cores.map(|v| v as i32)),
        cpu_threads: ActiveValue::Set(metrics.cpu_threads.map(|v| v as i32)),
        cpu_usage_percent: ActiveValue::Set(metrics.cpu_usage_percent),
        memory_total_bytes: ActiveValue::Set(metrics.memory_total_bytes.map(|v| v as i64)),
        memory_used_bytes: ActiveValue::Set(metrics.memory_used_bytes.map(|v| v as i64)),
        memory_usage_percent: ActiveValue::Set(metrics.memory_usage_percent),
        disk_total_bytes: ActiveValue::Set(metrics.disk_total_bytes.map(|v| v as i64)),
        disk_used_bytes: ActiveValue::Set(metrics.disk_used_bytes.map(|v| v as i64)),
        disk_usage_percent: ActiveValue::Set(metrics.disk_usage_percent),
        network_rx_bytes: ActiveValue::Set(metrics.network_rx_bytes.map(|v| v as i64)),
        network_tx_bytes: ActiveValue::Set(metrics.network_tx_bytes.map(|v| v as i64)),
        os_name: ActiveValue::Set(metrics.os_name),
        os_version: ActiveValue::Set(metrics.os_version),
        kernel_version: ActiveValue::Set(metrics.kernel_version),
        hostname: ActiveValue::Set(metrics.hostname),
        uptime_seconds: ActiveValue::Set(metrics.uptime_seconds.map(|v| v as i64)),
        custom_metrics: ActiveValue::Set(None),
    };

    new_metrics.insert(&state.db_conn).await.map_err(|e| {
        tracing::error!("Failed to store metrics: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::CREATED)
}

fn is_container_id(hostname: &str) -> bool {
    let h = hostname.trim();
    h.len() == 12 && h.chars().all(|c| c.is_ascii_hexdigit())
}

/// List all agents
pub async fn list_agents(
    State(state): State<AppState>,
    _perm: CanViewAgents,
) -> Result<impl IntoResponse, StatusCode> {
    let agents = agents::Entity::find()
        .order_by_desc(agents::Column::RegisteredAt)
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch agents: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response: Vec<AgentResponse> = agents
        .into_iter()
        .filter(|a| !is_container_id(&a.hostname))
        .map(Into::into)
        .collect();
    Ok(Json(response))
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
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch workloads for agent");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let mut response = Vec::with_capacity(rows.len());
    for workload in rows {
        let resource_group_cidr = match workload.resource_group_id {
            Some(rg_id) => resource_groups::Entity::find_by_id(rg_id)
                .one(&state.db_conn)
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
    _agent: AgentApiKey,
    State(state): State<AppState>,
    body: String,
) -> Result<impl IntoResponse, StatusCode> {
    let body_json: Option<serde_json::Value> = serde_json::from_str(&body).ok();
    match state
        .service_client
        .forward_to_scheduler(
            reqwest::Method::POST,
            "/internal/workloads/stats",
            body_json,
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
    _agent: AgentApiKey,
    State(state): State<AppState>,
    Path(workload_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let path = format!("/internal/workloads/{}/restart-ack", workload_id);
    match state
        .service_client
        .forward_to_scheduler(reqwest::Method::POST, &path, None, None)
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
        .all(&state.db_conn)
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
        .filter(
            workloads::Column::Status
                .eq("scheduled")
                .or(workloads::Column::Status.eq("running")),
        )
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

pub async fn uncordon_agent(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Path(agent_id): Path<Uuid>,
) -> Result<impl IntoResponse, StatusCode> {
    let agent = agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch agent for uncordon");
            StatusCode::INTERNAL_SERVER_ERROR
        })?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: agents::ActiveModel = agent.into();
    active.cordoned = ActiveValue::Set(false);
    active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to uncordon agent");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok(StatusCode::NO_CONTENT)
}

pub fn agents_routes() -> Router<AppState> {
    Router::new()
        .route("/agents", get(list_agents))
        .route("/agents/{id}", get(get_agent))
        .route("/agents/{id}/metrics", get(get_agent_metrics))
        .route("/agents/{id}/metrics/latest", get(get_agent_metrics_latest))
        .route("/agents/{id}/drain", post(drain_agent))
        .route("/agents/{id}/uncordon", post(uncordon_agent))
}

pub fn agents_unmetered_routes() -> Router<AppState> {
    Router::new()
        .route("/agents/metrics", post(receive_metrics))
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
