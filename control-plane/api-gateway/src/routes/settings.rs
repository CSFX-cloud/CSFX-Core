use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use entity::{entities::system_settings, SystemSettings};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde::{Deserialize, Serialize};
use serde_json::json;

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

    Ok((StatusCode::OK, Json(json!(AvatarFallbackResponse { style }))))
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

pub fn settings_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/settings/logs-retention",
            get(get_logs_retention).put(update_logs_retention),
        )
        .route(
            "/settings/avatar-fallback",
            get(get_avatar_fallback).put(update_avatar_fallback),
        )
}
