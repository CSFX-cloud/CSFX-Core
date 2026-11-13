use axum::{
    extract::{Query, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use entity::{entities::logs, Logs};
use sea_orm::{
    prelude::DateTimeWithTimeZone,
    sea_query::{Alias, Expr},
    ColumnTrait, EntityTrait, Order, PaginatorTrait, QueryFilter, QueryOrder, QuerySelect,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::{auth::rbac::CanViewLogs, cef::format_cef, AppState};

const KIND_USER: &str = "user";
const LOGS_TABLE: &str = "logs";
const AUDIT_TABLE: &str = "audit_log";
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
    pub cursor: Option<String>,
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
    pub total: Option<u64>,
    pub has_more: bool,
    pub next_cursor: Option<String>,
}

fn csv_values(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn apply_filters(params: &LogsQuery) -> sea_orm::Select<Logs> {
    let mut query = source(params);
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

fn source(params: &LogsQuery) -> sea_orm::Select<Logs> {
    let mut query = Logs::find();
    if params.kind.as_deref() == Some(KIND_USER) {
        QuerySelect::query(&mut query)
            .from_clear()
            .from_as(Alias::new(AUDIT_TABLE), Alias::new(LOGS_TABLE));
    }
    query
}

fn encode_cursor(entry: &logs::Model) -> String {
    format!("{}_{}", entry.last_seen_at.timestamp_micros(), entry.id)
}

fn decode_cursor(cursor: &str) -> Option<(DateTimeWithTimeZone, Uuid)> {
    let (micros, id) = cursor.split_once('_')?;
    let seen_at = chrono::DateTime::from_timestamp_micros(micros.parse().ok()?)?;
    Some((seen_at.fixed_offset(), Uuid::parse_str(id).ok()?))
}

fn database_error(error: sea_orm::DbErr) -> (StatusCode, Json<serde_json::Value>) {
    tracing::error!(error = %error, "failed to list logs");
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(json!({ "error": "database error" })),
    )
}

fn page_response(entries: Vec<logs::Model>, total: Option<u64>, has_more: bool) -> LogsResponse {
    let next_cursor = entries.last().filter(|_| has_more).map(encode_cursor);
    LogsResponse {
        entries: entries.into_iter().map(Into::into).collect(),
        total,
        has_more,
        next_cursor,
    }
}

async fn offset_page(
    state: &AppState,
    params: &LogsQuery,
    limit: u64,
) -> Result<LogsResponse, (StatusCode, Json<serde_json::Value>)> {
    let offset = params.offset.unwrap_or(0);
    let base_query = apply_filters(params);
    let total = base_query
        .clone()
        .count(&state.db_conn)
        .await
        .map_err(database_error)?;
    let entries = base_query
        .order_by_desc(logs::Column::LastSeenAt)
        .order_by_desc(logs::Column::Id)
        .offset(offset)
        .limit(limit)
        .all(&state.db_conn)
        .await
        .map_err(database_error)?;
    let has_more = offset + (entries.len() as u64) < total;
    Ok(page_response(entries, Some(total), has_more))
}

async fn keyset_page(
    state: &AppState,
    params: &LogsQuery,
    cursor: &str,
    limit: u64,
) -> Result<LogsResponse, (StatusCode, Json<serde_json::Value>)> {
    let mut query = apply_filters(params);
    if !cursor.is_empty() {
        let (seen_at, id) = decode_cursor(cursor).ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "invalid cursor" })),
            )
        })?;
        query = query.filter(
            Expr::tuple([
                Expr::col(logs::Column::LastSeenAt).into(),
                Expr::col(logs::Column::Id).into(),
            ])
            .lt(Expr::tuple([
                Expr::val(seen_at).into(),
                Expr::val(id).into(),
            ])),
        );
    }
    let mut entries = query
        .order_by_desc(logs::Column::LastSeenAt)
        .order_by_desc(logs::Column::Id)
        .limit(limit + 1)
        .all(&state.db_conn)
        .await
        .map_err(database_error)?;
    let has_more = entries.len() as u64 > limit;
    entries.truncate(limit as usize);
    Ok(page_response(entries, None, has_more))
}

pub async fn list_logs(
    CanViewLogs(_claims): CanViewLogs,
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let limit = params.limit.unwrap_or(DEFAULT_LIMIT).min(MAX_LIMIT);
    let response = match params.cursor.as_deref() {
        Some(cursor) => keyset_page(&state, &params, cursor, limit).await?,
        None => offset_page(&state, &params, limit).await?,
    };
    Ok((StatusCode::OK, Json(json!(response))))
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
    let rows: Vec<(Option<String>, i64)> = apply_filters(params)
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

async fn kind_counts(
    state: &AppState,
    params: &LogsQuery,
) -> Result<Vec<FacetCount>, (StatusCode, Json<serde_json::Value>)> {
    let unscoped = LogsQuery {
        kind: None,
        ..params.clone()
    };
    let audit = LogsQuery {
        kind: Some(KIND_USER.to_string()),
        ..params.clone()
    };
    let mut counts = count_by(state, &unscoped, logs::Column::Kind).await?;
    counts.retain(|facet| facet.value != KIND_USER);
    counts.extend(count_by(state, &audit, logs::Column::Kind).await?);
    counts.sort_by_key(|facet| std::cmp::Reverse(facet.count));
    Ok(counts)
}

pub async fn logs_facets(
    CanViewLogs(_claims): CanViewLogs,
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let facets = LogsFacets {
        kinds: kind_counts(&state, &params).await?,
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

    let rows: Vec<(i64, i64)> = apply_filters(&filtered)
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
    let entries = apply_filters(&params)
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
