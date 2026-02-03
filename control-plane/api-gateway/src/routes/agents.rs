use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use entity::entities::{agent_metrics, agents};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthenticatedUser;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentRegistration {
    pub agent_id: Uuid,
    pub name: String,
    pub hostname: String,
    pub os_type: String,
    pub os_version: String,
    pub architecture: String,
    pub agent_version: String,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RegistrationResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Heartbeat {
    pub agent_id: Uuid,
    pub timestamp: chrono::DateTime<chrono::Utc>,
    pub status: String,
}

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
        }
    }
}

/// Register a new agent or update existing one
pub async fn register_agent(
    State(state): State<AppState>,
    Json(registration): Json<AgentRegistration>,
) -> Result<impl IntoResponse, StatusCode> {
    // Check if agent already exists
    let existing_agent = agents::Entity::find()
        .filter(agents::Column::Id.eq(registration.agent_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(agent) = existing_agent {
        // Update existing agent
        let mut active_model: agents::ActiveModel = agent.into();
        active_model.name = ActiveValue::Set(registration.name);
        active_model.hostname = ActiveValue::Set(registration.hostname);
        active_model.os_type = ActiveValue::Set(registration.os_type);
        active_model.os_version = ActiveValue::Set(registration.os_version);
        active_model.architecture = ActiveValue::Set(registration.architecture);
        active_model.agent_version = ActiveValue::Set(registration.agent_version);
        active_model.status = ActiveValue::Set("online".to_string());
        active_model.last_heartbeat = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));
        active_model.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));
        if let Some(tags) = registration.tags {
            active_model.tags = ActiveValue::Set(Some(tags));
        }

        active_model.update(&state.db_conn).await.map_err(|e| {
            tracing::error!("Failed to update agent: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Ok(Json(RegistrationResponse {
            success: true,
            message: "Agent updated successfully".to_string(),
        }))
    } else {
        // Create new agent
        let new_agent = agents::ActiveModel {
            id: ActiveValue::Set(registration.agent_id),
            name: ActiveValue::Set(registration.name),
            hostname: ActiveValue::Set(registration.hostname),
            ip_address: ActiveValue::Set(None),
            agent_version: ActiveValue::Set(registration.agent_version),
            os_type: ActiveValue::Set(registration.os_type),
            os_version: ActiveValue::Set(registration.os_version),
            architecture: ActiveValue::Set(registration.architecture),
            status: ActiveValue::Set("online".to_string()),
            last_heartbeat: ActiveValue::Set(Some(chrono::Utc::now().naive_utc())),
            registered_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
            updated_at: ActiveValue::Set(None),
            organization_id: ActiveValue::Set(None),
            tags: ActiveValue::Set(registration.tags),
            capabilities: ActiveValue::Set(None),
        };

        new_agent.insert(&state.db_conn).await.map_err(|e| {
            tracing::error!("Failed to create agent: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Ok(Json(RegistrationResponse {
            success: true,
            message: "Agent registered successfully".to_string(),
        }))
    }
}

/// Receive heartbeat from agent
pub async fn heartbeat(
    State(state): State<AppState>,
    Json(heartbeat): Json<Heartbeat>,
) -> Result<impl IntoResponse, StatusCode> {
    let agent = agents::Entity::find()
        .filter(agents::Column::Id.eq(heartbeat.agent_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if let Some(agent) = agent {
        let mut active_model: agents::ActiveModel = agent.into();
        active_model.status = ActiveValue::Set(heartbeat.status);
        active_model.last_heartbeat = ActiveValue::Set(Some(heartbeat.timestamp.naive_utc()));
        active_model.updated_at = ActiveValue::Set(Some(chrono::Utc::now().naive_utc()));

        active_model.update(&state.db_conn).await.map_err(|e| {
            tracing::error!("Failed to update heartbeat: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
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

/// List all agents
pub async fn list_agents(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
) -> Result<impl IntoResponse, StatusCode> {
    let agents = agents::Entity::find()
        .order_by_desc(agents::Column::RegisteredAt)
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch agents: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let response: Vec<AgentResponse> = agents.into_iter().map(Into::into).collect();
    Ok(Json(response))
}

/// Get agent by ID
pub async fn get_agent(
    State(state): State<AppState>,
    _user: AuthenticatedUser,
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
    _user: AuthenticatedUser,
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

pub fn agents_routes() -> Router<AppState> {
    Router::new()
        // Public endpoints (for agents)
        .route("/agents/register", post(register_agent))
        .route("/agents/heartbeat", post(heartbeat))
        .route("/agents/metrics", post(receive_metrics))
        // Protected endpoints (for frontend)
        .route("/agents", get(list_agents))
        .route("/agents/:id", get(get_agent))
        .route("/agents/:id/metrics", get(get_agent_metrics))
}
