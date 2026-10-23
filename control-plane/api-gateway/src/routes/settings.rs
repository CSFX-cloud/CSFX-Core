use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{get, post},
    Router,
};
use entity::{entities::system_settings, SystemSettings};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared::ntfy::{self, NtfyConfig, NtfyError};

use crate::{
    auth::rbac::{AuthenticatedUser, CanManageLogs, CanManageSystem},
    AppState,
};

const RETENTION_KEY: &str = "logs.retention_days";
const MIN_RETENTION_DAYS: i64 = 1;
const MAX_RETENTION_DAYS: i64 = 365;

const AVATAR_FALLBACK_KEY: &str = "avatar.fallback_style";
const AVATAR_FALLBACK_STYLES: [&str; 2] = ["initials", "blobatar"];

#[derive(Debug, Serialize)]
pub struct LogsRetentionResponse {
    pub retention_days: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLogsRetentionRequest {
    pub retention_days: i64,
}

pub async fn get_logs_retention(
    CanManageLogs(_claims): CanManageLogs,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let retention_days = load_retention_days(&state).await?;
    Ok((
        StatusCode::OK,
        Json(json!(LogsRetentionResponse { retention_days })),
    ))
}

pub async fn update_logs_retention(
    CanManageLogs(_claims): CanManageLogs,
    State(state): State<AppState>,
    Json(req): Json<UpdateLogsRetentionRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if req.retention_days < MIN_RETENTION_DAYS || req.retention_days > MAX_RETENTION_DAYS {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!(
                "retention_days must be between {} and {}",
                MIN_RETENTION_DAYS, MAX_RETENTION_DAYS
            ) })),
        ));
    }

    let existing = SystemSettings::find_by_id(RETENTION_KEY)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to load logs retention setting");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let now = chrono::Utc::now().into();
    match existing {
        Some(setting) => {
            let mut active: system_settings::ActiveModel = setting.into();
            active.value = Set(json!(req.retention_days));
            active.updated_at = Set(now);
            active.update(&state.db_conn).await.map_err(|e| {
                tracing::error!(error = %e, "failed to update logs retention setting");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "database error" })),
                )
            })?;
        }
        None => {
            let active = system_settings::ActiveModel {
                key: Set(RETENTION_KEY.to_string()),
                value: Set(json!(req.retention_days)),
                updated_at: Set(now),
            };
            active.insert(&state.db_conn).await.map_err(|e| {
                tracing::error!(error = %e, "failed to insert logs retention setting");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "database error" })),
                )
            })?;
        }
    }

    Ok((
        StatusCode::OK,
        Json(json!(LogsRetentionResponse {
            retention_days: req.retention_days
        })),
    ))
}

pub async fn load_retention_days(
    state: &AppState,
) -> Result<i64, (StatusCode, Json<serde_json::Value>)> {
    let setting = SystemSettings::find_by_id(RETENTION_KEY)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to load logs retention setting");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    Ok(setting
        .and_then(|s| s.value.as_i64())
        .unwrap_or(MAX_RETENTION_DAYS.min(30)))
}

#[derive(Debug, Serialize)]
pub struct AvatarFallbackResponse {
    pub style: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAvatarFallbackRequest {
    pub style: String,
}

pub async fn get_avatar_fallback(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let setting = SystemSettings::find_by_id(AVATAR_FALLBACK_KEY)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to load avatar fallback setting");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let style = setting
        .and_then(|s| s.value.as_str().map(str::to_string))
        .unwrap_or_else(|| "initials".to_string());

    Ok((
        StatusCode::OK,
        Json(json!(AvatarFallbackResponse { style })),
    ))
}

pub async fn update_avatar_fallback(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Json(req): Json<UpdateAvatarFallbackRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    if !AVATAR_FALLBACK_STYLES.contains(&req.style.as_str()) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!(
                "style must be one of {}",
                AVATAR_FALLBACK_STYLES.join(", ")
            ) })),
        ));
    }

    let existing = SystemSettings::find_by_id(AVATAR_FALLBACK_KEY)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to load avatar fallback setting");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let now = chrono::Utc::now().into();
    match existing {
        Some(setting) => {
            let mut active: system_settings::ActiveModel = setting.into();
            active.value = Set(json!(req.style));
            active.updated_at = Set(now);
            active.update(&state.db_conn).await.map_err(|e| {
                tracing::error!(error = %e, "failed to update avatar fallback setting");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "database error" })),
                )
            })?;
        }
        None => {
            let active = system_settings::ActiveModel {
                key: Set(AVATAR_FALLBACK_KEY.to_string()),
                value: Set(json!(req.style)),
                updated_at: Set(now),
            };
            active.insert(&state.db_conn).await.map_err(|e| {
                tracing::error!(error = %e, "failed to insert avatar fallback setting");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "database error" })),
                )
            })?;
        }
    }

    Ok((
        StatusCode::OK,
        Json(json!(AvatarFallbackResponse { style: req.style })),
    ))
}

#[derive(Debug, Serialize)]
pub struct NotificationSettingsResponse {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub token_set: bool,
    pub min_severity: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateNotificationSettingsRequest {
    pub enabled: bool,
    pub server_url: String,
    pub topic: String,
    pub token: Option<String>,
    pub min_severity: String,
}

fn ntfy_error(error: NtfyError) -> (StatusCode, Json<serde_json::Value>) {
    match error {
        NtfyError::Invalid(reason) => (StatusCode::BAD_REQUEST, Json(json!({ "error": reason }))),
        NtfyError::Request(e) => {
            tracing::warn!(error = %e, "ntfy request failed");
            (
                StatusCode::BAD_GATEWAY,
                Json(json!({ "error": format!("ntfy request failed: {e}") })),
            )
        }
        NtfyError::Database(e) => {
            tracing::error!(error = %e, "failed to access notification settings");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        }
    }
}

fn notification_response(config: NtfyConfig) -> Json<serde_json::Value> {
    Json(json!(NotificationSettingsResponse {
        enabled: config.enabled,
        server_url: config.server_url,
        topic: config.topic,
        token_set: config.token.is_some(),
        min_severity: config.min_severity,
    }))
}

pub async fn get_notification_settings(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let config = ntfy::load_config(&state.db_conn)
        .await
        .map_err(ntfy_error)?;
    Ok((StatusCode::OK, notification_response(config)))
}

pub async fn update_notification_settings(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
    Json(req): Json<UpdateNotificationSettingsRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let current = ntfy::load_config(&state.db_conn)
        .await
        .map_err(ntfy_error)?;
    let token = match req.token {
        Some(token) if token.is_empty() => None,
        Some(token) => Some(token),
        None => current.token,
    };
    let config = NtfyConfig {
        enabled: req.enabled,
        server_url: req.server_url,
        topic: req.topic,
        token,
        min_severity: req.min_severity,
    };
    config.validate().map_err(ntfy_error)?;
    ntfy::save_config(&state.db_conn, &config)
        .await
        .map_err(ntfy_error)?;
    Ok((StatusCode::OK, notification_response(config)))
}

pub async fn send_test_notification(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let config = ntfy::load_config(&state.db_conn)
        .await
        .map_err(ntfy_error)?;
    config.validate().map_err(ntfy_error)?;
    config
        .publish(
            "test notification",
            "csfx notifications are working",
            "info",
        )
        .await
        .map_err(ntfy_error)?;
    Ok(StatusCode::NO_CONTENT)
}

pub fn settings_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/settings/logs-retention",
            get(get_logs_retention).put(update_logs_retention),
        )
        .route(
            "/admin/settings/notifications",
            get(get_notification_settings).put(update_notification_settings),
        )
        .route(
            "/admin/settings/notifications/test",
            post(send_test_notification),
        )
        .route(
            "/settings/avatar-fallback",
            get(get_avatar_fallback).put(update_avatar_fallback),
        )
}
