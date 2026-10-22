use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::Json,
    routing::get,
    Router,
};
use entity::entities::{agent_metrics, agents};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, EntityTrait, QueryFilter, QueryOrder, QueryResult,
    Statement,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthenticatedUser;
use crate::system_collector::{LocalSystemCollector, LocalSystemMetrics};
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemInfoResponse {
    pub hostname: String,
    pub os_name: String,
    pub os_version: String,
    pub kernel_version: String,
    pub uptime_seconds: u64,
    pub cpu_model: String,
    pub cpu_cores: u32,
    pub cpu_threads: u32,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemMetricsResponse {
    pub metrics: LocalSystemMetrics,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct NodeStats {
    pub agent_id: String,
    pub name: String,
    pub hostname: String,
    pub status: String,
    pub cpu_usage_percent: Option<f32>,
    pub memory_total_bytes: Option<i64>,
    pub memory_used_bytes: Option<i64>,
    pub disk_total_bytes: Option<i64>,
    pub disk_used_bytes: Option<i64>,
    pub uptime_seconds: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterStats {
    pub node_count: usize,
    pub online_count: usize,
    pub total_cpu_cores: i64,
    pub avg_cpu_usage_percent: f32,
    pub total_memory_bytes: i64,
    pub used_memory_bytes: i64,
    pub total_disk_bytes: i64,
    pub used_disk_bytes: i64,
    pub nodes: Vec<NodeStats>,
}

fn is_container_id(hostname: &str) -> bool {
    if std::env::var("CSFX_INCLUDE_CONTAINERS").as_deref() == Ok("true") {
        return false;
    }
    let trimmed = hostname.trim();
    trimmed.len() == 12 && trimmed.chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthHistoryPoint {
    pub bucket: String,
    pub online_count: i64,
}

#[derive(Debug, Serialize)]
pub struct UsageHistoryPoint {
    pub bucket: String,
    pub cpu: f64,
    pub memory: f64,
    pub disk: f64,
}

#[derive(Debug, Deserialize)]
pub struct HistoryRangeQuery {
    pub range: Option<String>,
    pub agent_id: Option<Uuid>,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system/health", get(health_check))
        .route("/system/info", get(get_system_info))
        .route("/system/metrics", get(get_system_metrics))
        .route("/system/stats", get(get_cluster_stats))
        .route("/system/stats/history", get(get_health_history))
        .route("/system/stats/usage", get(get_usage_history))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "csfx-core-backend"
    }))
}

async fn get_system_info(
    _auth: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Json<SystemInfoResponse> {
    let collector = LocalSystemCollector::new();
    let metrics = collector.collect();

    Json(SystemInfoResponse {
        hostname: metrics.hostname,
        os_name: metrics.os_name,
        os_version: metrics.os_version,
        kernel_version: metrics.kernel_version,
        uptime_seconds: metrics.uptime_seconds,
        cpu_model: metrics.cpu_model,
        cpu_cores: metrics.cpu_cores,
        cpu_threads: metrics.cpu_threads,
    })
}

async fn get_system_metrics(
    _auth: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Json<SystemMetricsResponse> {
    let collector = LocalSystemCollector::new();
    let metrics = collector.collect();
    Json(SystemMetricsResponse { metrics })
}

async fn get_cluster_stats(
    _auth: AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<ClusterStats>, StatusCode> {
    let all_agents = agents::Entity::find()
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch agents for cluster stats");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let physical_agents: Vec<_> = all_agents
        .into_iter()
        .filter(|a| !is_container_id(&a.hostname))
        .collect();

    let mut nodes = Vec::with_capacity(physical_agents.len());
    let mut total_cpu_cores: i64 = 0;
    let mut cpu_usage_sum: f32 = 0.0;
    let mut cpu_usage_count: usize = 0;
    let mut total_memory: i64 = 0;
    let mut used_memory: i64 = 0;
    let mut total_disk: i64 = 0;
    let mut used_disk: i64 = 0;
    let mut online_count: usize = 0;

    for agent in &physical_agents {
        if agent.status.eq_ignore_ascii_case("online") {
            online_count += 1;
        }

        let latest_metric = agent_metrics::Entity::find()
            .filter(agent_metrics::Column::AgentId.eq(agent.id))
            .order_by_desc(agent_metrics::Column::Timestamp)
            .one(&state.db_conn)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, agent_id = %agent.id, "failed to fetch agent metrics");
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        let (cpu_pct, mem_total, mem_used, disk_total, disk_used, uptime, cores) = latest_metric
            .as_ref()
            .map(|m| {
                (
                    m.cpu_usage_percent,
                    m.memory_total_bytes,
                    m.memory_used_bytes,
                    m.disk_total_bytes,
                    m.disk_used_bytes,
                    m.uptime_seconds,
                    m.cpu_cores,
                )
            })
            .unwrap_or_default();

        if let Some(c) = cores {
            total_cpu_cores += c as i64;
        }
        if let Some(p) = cpu_pct {
            cpu_usage_sum += p;
            cpu_usage_count += 1;
        }
        total_memory += mem_total.unwrap_or(0);
        used_memory += mem_used.unwrap_or(0);
        total_disk += disk_total.unwrap_or(0);
        used_disk += disk_used.unwrap_or(0);

        nodes.push(NodeStats {
            agent_id: agent.id.to_string(),
            name: agent.name.clone(),
            hostname: agent.hostname.clone(),
            status: agent.status.clone(),
            cpu_usage_percent: cpu_pct,
            memory_total_bytes: mem_total,
            memory_used_bytes: mem_used,
            disk_total_bytes: disk_total,
            disk_used_bytes: disk_used,
            uptime_seconds: uptime,
        });
    }

    let avg_cpu = if cpu_usage_count > 0 {
        cpu_usage_sum / cpu_usage_count as f32
    } else {
        0.0
    };

    Ok(Json(ClusterStats {
        node_count: physical_agents.len(),
        online_count,
        total_cpu_cores,
        avg_cpu_usage_percent: avg_cpu,
        total_memory_bytes: total_memory,
        used_memory_bytes: used_memory,
        total_disk_bytes: total_disk,
        used_disk_bytes: used_disk,
        nodes,
    }))
}

async fn get_health_history(
    _auth: AuthenticatedUser,
    Query(params): Query<HistoryRangeQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<HealthHistoryPoint>>, StatusCode> {
    let range = params.range.as_deref().unwrap_or("1h");

    let (interval_str, bucket_seconds, limit_n) = match range {
        "24h" => ("24 hours", 3600i64, 24usize),
        "7d" => ("7 days", 3600i64, 168usize),
        "30d" => ("30 days", 21600i64, 120usize),
        _ => ("1 hour", 300i64, 12usize),
    };

    let sql = format!(
        r#"
        SELECT
            to_timestamp(floor(extract(epoch from timestamp) / {bucket_seconds}) * {bucket_seconds}) AS bucket,
            COUNT(DISTINCT agent_id) AS online_count
        FROM agent_metrics
        WHERE timestamp >= NOW() - INTERVAL '{interval}'
        GROUP BY bucket
        ORDER BY bucket ASC
        LIMIT {limit}
        "#,
        bucket_seconds = bucket_seconds,
        interval = interval_str,
        limit = limit_n,
    );

    let rows = state
        .db_conn
        .query_all(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to query health history");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let points = rows
        .into_iter()
        .map(|row| {
            let bucket: chrono::NaiveDateTime = row.try_get("", "bucket").unwrap_or_default();
            let online_count: i64 = row.try_get("", "online_count").unwrap_or(0);
            HealthHistoryPoint {
                bucket: bucket.format("%Y-%m-%dT%H:%M:%S").to_string(),
                online_count,
            }
        })
        .collect();

    Ok(Json(points))
}

fn usage_range(range: Option<&str>) -> (&'static str, i64) {
    match range {
        Some("7d") => ("7 days", 3600),
        Some("24h") => ("24 hours", 900),
        _ => ("1 hour", 60),
    }
}

fn usage_history_sql(interval: &str, bucket_seconds: i64, agent_id: Option<Uuid>) -> String {
    let agent_filter = agent_id
        .map(|id| format!("AND agent_id = '{id}'"))
        .unwrap_or_default();
    format!(
        r#"
        SELECT
            to_timestamp(floor(extract(epoch from timestamp) / {bucket_seconds}) * {bucket_seconds}) AS bucket,
            AVG(cpu_usage_percent) AS cpu,
            AVG(memory_usage_percent) AS memory,
            AVG(disk_usage_percent) AS disk
        FROM agent_metrics
        WHERE timestamp >= NOW() - INTERVAL '{interval}' {agent_filter}
        GROUP BY bucket
        ORDER BY bucket ASC
        "#
    )
}

fn usage_point(row: QueryResult) -> UsageHistoryPoint {
    let bucket: chrono::NaiveDateTime = row.try_get("", "bucket").unwrap_or_default();
    let percent = |column: &str| row.try_get::<Option<f64>>("", column).ok().flatten().unwrap_or(0.0);
    UsageHistoryPoint {
        bucket: bucket.format("%Y-%m-%dT%H:%M:%S").to_string(),
        cpu: percent("cpu"),
        memory: percent("memory"),
        disk: percent("disk"),
    }
}

async fn get_usage_history(
    _auth: AuthenticatedUser,
    Query(params): Query<HistoryRangeQuery>,
    State(state): State<AppState>,
) -> Result<Json<Vec<UsageHistoryPoint>>, StatusCode> {
    let (interval, bucket_seconds) = usage_range(params.range.as_deref());
    let sql = usage_history_sql(interval, bucket_seconds, params.agent_id);
    let rows = state
        .db_conn
        .query_all(Statement::from_string(DatabaseBackend::Postgres, sql))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to query usage history");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(Json(rows.into_iter().map(usage_point).collect()))
}
