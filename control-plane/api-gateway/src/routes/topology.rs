use std::collections::HashMap;

use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use chrono::{Duration, Utc};
use entity::entities::{agent_metrics, agents, resource_groups, volumes, workloads};
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter, QueryOrder};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::rbac::{CanViewAgents, CanViewResourceGroups, CanViewVolumes, CanViewWorkloads},
    AppState,
};

const CLUSTER_NODE_ID: &str = "cluster";
const METRICS_WINDOW_MINUTES: i64 = 5;
const DELETED_STATUS: &str = "deleted";

type ApiError = (StatusCode, Json<Value>);

#[derive(Serialize)]
pub struct TopologyNode {
    pub id: String,
    pub kind: &'static str,
    pub label: String,
    pub data: Value,
}

#[derive(Serialize)]
pub struct TopologyEdge {
    pub id: String,
    pub source: String,
    pub target: String,
    pub kind: &'static str,
}

#[derive(Serialize)]
pub struct TopologyResponse {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
}

fn db_error(error: impl std::fmt::Display) -> ApiError {
    tracing::error!(error = %error, "failed to build topology");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "database error" })),
    )
}

fn org_scope<C: ColumnTrait>(column: C, org_id: Option<Uuid>) -> Condition {
    let scope = Condition::any().add(column.is_null());
    match org_id {
        Some(id) => scope.add(column.eq(id)),
        None => scope,
    }
}

fn edge(kind: &'static str, source: String, target: String) -> TopologyEdge {
    TopologyEdge {
        id: format!("{kind}:{source}:{target}"),
        source,
        target,
        kind,
    }
}

async fn latest_metrics(state: &AppState) -> Result<HashMap<Uuid, agent_metrics::Model>, ApiError> {
    let since = (Utc::now() - Duration::minutes(METRICS_WINDOW_MINUTES)).naive_utc();
    let rows = agent_metrics::Entity::find()
        .filter(agent_metrics::Column::Timestamp.gte(since))
        .order_by_asc(agent_metrics::Column::Timestamp)
        .all(&state.db_conn)
        .await
        .map_err(db_error)?;
    Ok(rows.into_iter().map(|row| (row.agent_id, row)).collect())
}

fn agent_node(agent: agents::Model, metrics: Option<&agent_metrics::Model>) -> TopologyNode {
    TopologyNode {
        id: format!("agent:{}", agent.id),
        kind: "agent",
        label: agent.name,
        data: json!({
            "id": agent.id,
            "hostname": agent.hostname,
            "ip_address": agent.ip_address,
            "role": agent.role,
            "os_type": agent.os_type,
            "status": agent.status,
            "cordoned": agent.cordoned,
            "agent_version": agent.agent_version,
            "cpu_percent": metrics.and_then(|m| m.cpu_usage_percent),
            "memory_percent": metrics.and_then(|m| m.memory_usage_percent),
            "disk_percent": metrics.and_then(|m| m.disk_usage_percent),
        }),
    }
}

fn workload_node(workload: &workloads::Model) -> TopologyNode {
    TopologyNode {
        id: format!("workload:{}", workload.id),
        kind: "workload",
        label: workload.name.clone(),
        data: json!({
            "id": workload.id,
            "image": workload.image,
            "status": workload.status,
            "runtime_class": workload.runtime_class,
            "resource_group_id": workload.resource_group_id,
            "cpu_percent": workload.cpu_usage_percent,
            "memory_bytes": workload.memory_usage_bytes,
        }),
    }
}

fn workload_edge(workload: &workloads::Model) -> TopologyEdge {
    let target = format!("workload:{}", workload.id);
    match workload.assigned_agent_id {
        Some(agent_id) => edge("runs", format!("agent:{agent_id}"), target),
        None => edge("unscheduled", CLUSTER_NODE_ID.to_string(), target),
    }
}

fn volume_node(volume: &volumes::Model) -> TopologyNode {
    TopologyNode {
        id: format!("volume:{}", volume.id),
        kind: "volume",
        label: volume.name.clone(),
        data: json!({
            "id": volume.id,
            "size_gb": volume.size_gb,
            "status": volume.status,
            "resource_group_id": volume.resource_group_id,
        }),
    }
}

fn volume_edge(volume: &volumes::Model) -> TopologyEdge {
    let target = format!("volume:{}", volume.id);
    match (volume.attached_to_workload, volume.attached_to_agent) {
        (Some(workload_id), _) => edge("mounts", format!("workload:{workload_id}"), target),
        (None, Some(agent_id)) => edge("attached", format!("agent:{agent_id}"), target),
        (None, None) => edge("unattached", CLUSTER_NODE_ID.to_string(), target),
    }
}

fn resource_group_node(group: resource_groups::Model) -> TopologyNode {
    TopologyNode {
        id: format!("rg:{}", group.id),
        kind: "resource_group",
        label: group.name,
        data: json!({
            "id": group.id,
            "internal_cidr": group.internal_cidr,
            "status": group.status,
            "color": group.color,
            "icon": group.icon,
        }),
    }
}

pub async fn get_topology(
    _agents: CanViewAgents,
    _workloads: CanViewWorkloads,
    _volumes: CanViewVolumes,
    _groups: CanViewResourceGroups,
    State(state): State<AppState>,
) -> Result<Json<TopologyResponse>, ApiError> {
    let db = &state.db_conn;
    let org_id = state.default_org_id;

    let agent_rows = agents::Entity::find().all(db).await.map_err(db_error)?;
    let workload_rows = workloads::Entity::find()
        .filter(workloads::Column::Status.ne(DELETED_STATUS))
        .filter(org_scope(workloads::Column::OrganizationId, org_id))
        .all(db)
        .await
        .map_err(db_error)?;
    let volume_rows = volumes::Entity::find()
        .filter(org_scope(volumes::Column::OrganizationId, org_id))
        .all(db)
        .await
        .map_err(db_error)?;
    let group_rows = resource_groups::Entity::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .all(db)
        .await
        .map_err(db_error)?;
    let metrics = latest_metrics(&state).await?;

    let mut nodes = vec![TopologyNode {
        id: CLUSTER_NODE_ID.to_string(),
        kind: "cluster",
        label: "Cluster".to_string(),
        data: json!({}),
    }];
    let mut edges = Vec::new();

    for agent in agent_rows {
        edges.push(edge(
            "member",
            CLUSTER_NODE_ID.to_string(),
            format!("agent:{}", agent.id),
        ));
        let agent_metrics = metrics.get(&agent.id);
        nodes.push(agent_node(agent, agent_metrics));
    }
    for workload in &workload_rows {
        nodes.push(workload_node(workload));
        edges.push(workload_edge(workload));
    }
    for volume in &volume_rows {
        nodes.push(volume_node(volume));
        edges.push(volume_edge(volume));
    }
    nodes.extend(group_rows.into_iter().map(resource_group_node));

    Ok(Json(TopologyResponse { nodes, edges }))
}

pub fn topology_routes() -> Router<AppState> {
    Router::new().route("/topology", get(get_topology))
}
