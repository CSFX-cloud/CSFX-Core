use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use entity::{entities::logs, Logs};
use sea_orm::{
    sea_query::Expr, ColumnTrait, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{auth::rbac::CanViewLogs, cef::format_cef, AppState};

const DEFAULT_LIMIT: u64 = 100;
const MAX_LIMIT: u64 = 500;
const EXPORT_LIMIT: u64 = 10_000;
const TREND_BUCKETS: i64 = 60;
const DEFAULT_TREND_WINDOW_HOURS: i64 = 24;

#[derive(Clone, Debug, Default, Deserialize)]
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
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub from: Option<chrono::DateTime<chrono::Utc>>,
    pub to: Option<chrono::DateTime<chrono::Utc>>,
    pub q: Option<String>,
    pub limit: Option<u64>,
    pub offset: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct LogEntryResponse {
    #[serde(flatten)]
    pub entry: logs::Model,
    pub cef: String,
}

impl From<logs::Model> for LogEntryResponse {
    fn from(entry: logs::Model) -> Self {
        let cef = format_cef(&entry);
        Self { entry, cef }
    }
}

#[derive(Debug, Serialize)]
pub struct LogsResponse {
    pub entries: Vec<LogEntryResponse>,
    pub total: u64,
    pub has_more: bool,
}

fn csv_values(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn apply_filters(mut query: sea_orm::Select<Logs>, params: &LogsQuery) -> sea_orm::Select<Logs> {
    if let Some(service) = &params.service {
        query = query.filter(logs::Column::Service.is_in(csv_values(service)));
    }
    if let Some(level) = &params.level {
        query = query.filter(logs::Column::Level.is_in(csv_values(level)));
    }
    if let Some(classification) = &params.classification {
        query = query.filter(logs::Column::Classification.is_in(csv_values(classification)));
    }
    if let Some(target_type) = &params.target_type {
        query = query.filter(logs::Column::TargetType.is_in(csv_values(target_type)));
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
        query = query.filter(logs::Column::Action.is_in(csv_values(action)));
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
    let entries: Vec<LogEntryResponse> = entries.into_iter().map(Into::into).collect();

    Ok((
        StatusCode::OK,
        Json(json!(LogsResponse {
            entries,
            total,
            has_more,
        })),
    ))
}

#[derive(Debug, Serialize)]
pub struct FacetCount {
    pub value: String,
    pub count: i64,
}

#[derive(Debug, Serialize)]
pub struct LogsFacets {
    pub kinds: Vec<FacetCount>,
    pub classifications: Vec<FacetCount>,
    pub services: Vec<FacetCount>,
    pub target_types: Vec<FacetCount>,
    pub actions: Vec<FacetCount>,
}

fn facet_error(error: sea_orm::DbErr) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!(error = %error, "failed to compute log facets");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "database error" })),
    )
}

async fn count_by(
    state: &AppState,
    params: &LogsQuery,
    column: logs::Column,
) -> Result<Vec<FacetCount>, (StatusCode, Json<serde_json::Value>)> {
    let rows: Vec<(Option<String>, i64)> = apply_filters(Logs::find(), params)
        .select_only()
        .column(column)
        .column_as(logs::Column::Id.count(), "count")
        .group_by(column)
        .order_by_desc(logs::Column::Id.count())
        .into_tuple()
        .all(&state.db_conn)
        .await
        .map_err(facet_error)?;

    Ok(rows
        .into_iter()
        .filter_map(|(value, count)| {
            Some(FacetCount {
                value: value?,
                count,
            })
        })
        .collect())
}

pub async fn logs_facets(
    CanViewLogs(_claims): CanViewLogs,
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let facets = LogsFacets {
        kinds: count_by(
            &state,
            &LogsQuery {
                kind: None,
                ..params.clone()
            },
            logs::Column::Kind,
        )
        .await?,
        classifications: count_by(
            &state,
            &LogsQuery {
                classification: None,
                ..params.clone()
            },
            logs::Column::Classification,
        )
        .await?,
        services: count_by(
            &state,
            &LogsQuery {
                service: None,
                ..params.clone()
            },
            logs::Column::Service,
        )
        .await?,
        target_types: count_by(
            &state,
            &LogsQuery {
                target_type: None,
                ..params.clone()
            },
            logs::Column::TargetType,
        )
        .await?,
        actions: count_by(
            &state,
            &LogsQuery {
                action: None,
                ..params.clone()
            },
            logs::Column::Action,
        )
        .await?,
    };

    Ok((StatusCode::OK, Json(json!(facets))))
}

#[derive(Debug, Serialize)]
pub struct TrendPoint {
    pub start: chrono::DateTime<chrono::Utc>,
    pub count: i64,
}

fn trend_points(
    from: chrono::DateTime<chrono::Utc>,
    bucket_seconds: i64,
    rows: Vec<(i64, i64)>,
) -> Vec<TrendPoint> {
    let counts: std::collections::HashMap<i64, i64> = rows.into_iter().collect();
    (0..TREND_BUCKETS)
        .map(|index| TrendPoint {
            start: from + chrono::Duration::seconds(index * bucket_seconds),
            count: counts.get(&index).copied().unwrap_or(0),
        })
        .collect()
}

pub async fn logs_trend(
    CanViewLogs(_claims): CanViewLogs,
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let to = params.to.unwrap_or_else(chrono::Utc::now);
    let from = params
        .from
        .unwrap_or(to - chrono::Duration::hours(DEFAULT_TREND_WINDOW_HOURS));
    let span = (to - from).num_seconds().max(TREND_BUCKETS);
    let bucket_seconds = (span + TREND_BUCKETS - 1) / TREND_BUCKETS;
    let filtered = LogsQuery {
        from: Some(from),
        to: Some(to),
        ..params
    };
    let bucket = Expr::cust(format!(
        "FLOOR((EXTRACT(EPOCH FROM last_seen_at) - {}) / {})::bigint",
        from.timestamp(),
        bucket_seconds
    ));

    let rows: Vec<(i64, i64)> = apply_filters(Logs::find(), &filtered)
        .select_only()
        .expr_as(bucket.clone(), "bucket")
        .column_as(logs::Column::Id.count(), "count")
        .group_by(bucket)
        .into_tuple()
        .all(&state.db_conn)
        .await
        .map_err(facet_error)?;

    Ok((
        StatusCode::OK,
        Json(json!(trend_points(from, bucket_seconds, rows))),
    ))
}

pub async fn export_logs(
    CanViewLogs(_claims): CanViewLogs,
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let entries = apply_filters(Logs::find(), &params)
        .order_by(logs::Column::LastSeenAt, Order::Desc)
        .limit(EXPORT_LIMIT)
        .all(&state.db_conn)
        .await
        .map_err(facet_error)?;

    let body = entries
        .iter()
        .map(format_cef)
        .collect::<Vec<_>>()
        .join("\n");

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=\"csfx-logs.cef\"",
            ),
        ],
        body,
    ))
}

pub fn logs_routes() -> Router<AppState> {
    Router::new()
        .route("/logs", get(list_logs))
        .route("/logs/export", get(export_logs))
        .route("/logs/facets", get(logs_facets))
        .route("/logs/trend", get(logs_trend))
}
