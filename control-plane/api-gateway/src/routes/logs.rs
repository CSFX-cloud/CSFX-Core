use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use entity::{entities::logs, Logs};
use sea_orm::{
    ColumnTrait, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{auth::rbac::CanViewLogs, AppState};

const DEFAULT_LIMIT: u64 = 100;
const MAX_LIMIT: u64 = 500;

#[derive(Debug, Deserialize)]
pub struct LogsQuery {
    pub service: Option<String>,
    pub level: Option<String>,
    pub classification: Option<String>,
    pub agent_id: Option<Uuid>,
    pub workload_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub kind: Option<String>,
    pub actor_user_id: Option<Uuid>,
    pub action: Option<String>,
    pub target_id: Option<String>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub q: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub entries: Vec<logs::Model>,
    pub total: u64,
    pub has_more: bool,
}

fn apply_filters(mut query: sea_orm::Select<Logs>, params: &LogsQuery) -> sea_orm::Select<Logs> {
    if let Some(service) = &params.service {
        query = query.filter(logs::Column::Service.eq(service));
    }
    if let Some(level) = &params.level {
        query = query.filter(logs::Column::Level.eq(level));
    }
    if let Some(classification) = &params.classification {
        query = query.filter(logs::Column::Classification.eq(classification));
    }
    if let Some(agent_id) = params.agent_id {
        query = query.filter(logs::Column::AgentId.eq(agent_id));
    }
    if let Some(workload_id) = params.workload_id {
        query = query.filter(logs::Column::WorkloadId.eq(workload_id));
    }
    if let Some(organization_id) = params.organization_id {
        query = query.filter(logs::Column::OrganizationId.eq(organization_id));
    }
    if let Some(kind) = &params.kind {
        query = query.filter(logs::Column::Kind.eq(kind));
    }
    if let Some(actor_user_id) = params.actor_user_id {
        query = query.filter(logs::Column::ActorUserId.eq(actor_user_id));
    }
    if let Some(action) = &params.action {
        query = query.filter(logs::Column::Action.eq(action));
    }
    if let Some(target_id) = &params.target_id {
        query = query.filter(logs::Column::TargetId.eq(target_id));
    }
    if let Some(from) = params.from {
        query = query.filter(logs::Column::LastSeenAt.gte(from));
    }
    if let Some(to) = params.to {
        query = query.filter(logs::Column::LastSeenAt.lte(to));
    }
    if let Some(q) = &params.q {
        query = query.filter(logs::Column::Message.contains(q));
    }
    query
}

pub async fn list_logs(
    CanViewLogs(_claims): CanViewLogs,
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let offset = params.offset.unwrap_or(0);

    let base_query = apply_filters(Logs::find(), &params);

    let total = base_query
        .clone()
        .count(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to count logs");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let entries = base_query
        .order_by(logs::Column::LastSeenAt, Order::Desc)
        .offset(offset)
        .limit(limit)
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list logs");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let has_more = offset + (entries.len() as u64) < total;

    Ok((
        StatusCode::OK,
        Json(json!(LogsResponse {
            entries,
            total,
            has_more,
        })),
    ))
}

pub fn logs_routes() -> Router<AppState> {
    Router::new().route("/logs", get(list_logs))
}
