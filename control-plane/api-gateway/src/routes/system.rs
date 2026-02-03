use axum::{extract::State, response::Json, routing::get, Router};
use serde::{Deserialize, Serialize};

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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/system/health", get(health_check))
        .route("/system/info", get(get_system_info))
        .route("/system/metrics", get(get_system_metrics))
}

/// Health check endpoint
///
/// Simple endpoint to check if the service is running
async fn health_check() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "healthy",
        "service": "csf-core-backend"
    }))
}

/// Get local system information
///
/// Returns static system information like hostname, OS, CPU details
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

/// Get current system metrics
///
/// Returns real-time metrics like CPU usage, memory usage, disk usage, network stats
async fn get_system_metrics(
    _auth: AuthenticatedUser,
    State(_state): State<AppState>,
) -> Json<SystemMetricsResponse> {
    let collector = LocalSystemCollector::new();
    let metrics = collector.collect();

    Json(SystemMetricsResponse { metrics })
}
