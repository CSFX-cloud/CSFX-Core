use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use chrono::Utc;
use entity::{entities::system_settings, SystemSettings};
use sea_orm::{sea_query::OnConflict, ActiveValue::Set, EntityTrait};
use serde_json::{json, Value};

use super::{
    cluster::{error, internal, ApiError},
    masters::registered_master,
};
use crate::{
    auth::{agent::AgentApiKey, rbac::CanManageSystem},
    cluster_bundle::ClusterVip,
    AppState,
};

const VIP_KEY: &str = "cluster.vip";

pub async fn load_vip(state: &AppState) -> Result<Option<ClusterVip>, ApiError> {
    SystemSettings::find_by_id(VIP_KEY)
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .map(|setting| serde_json::from_value(setting.value))
        .transpose()
        .map_err(internal)
}

pub async fn effective_vip(state: &AppState) -> Result<Option<ClusterVip>, ApiError> {
    Ok(load_vip(state).await?.or_else(ClusterVip::from_env))
}

async fn get_vip(
    CanManageSystem(_claims): CanManageSystem,
    State(state): State<AppState>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!({ "vip": effective_vip(&state).await? })))
}

async fn put_vip(
    CanManageSystem(claims): CanManageSystem,
    State(state): State<AppState>,
    Json(vip): Json<ClusterVip>,
) -> Result<Json<Value>, ApiError> {
    let vip = vip
        .validated()
        .map_err(|message| error(StatusCode::BAD_REQUEST, message))?;
    let row = system_settings::ActiveModel {
        key: Set(VIP_KEY.to_string()),
        value: Set(serde_json::to_value(&vip).map_err(internal)?),
        updated_at: Set(Utc::now().into()),
    };
    let on_conflict = OnConflict::column(system_settings::Column::Key)
        .update_columns([
            system_settings::Column::Value,
            system_settings::Column::UpdatedAt,
        ])
        .to_owned();
    SystemSettings::insert(row)
        .on_conflict(on_conflict)
        .exec(&state.db_conn)
        .await
        .map_err(internal)?;
    tracing::info!(
        target: "csfx::audit",
        event = "cluster.vip_updated",
        user_id = %claims.user_id,
        address = %vip.address
    );
    Ok(Json(json!({ "vip": vip })))
}

async fn agent_vip(
    agent: AgentApiKey,
    State(state): State<AppState>,
) -> Result<Json<Option<ClusterVip>>, ApiError> {
    registered_master(&state, agent.agent_id).await?;
    Ok(Json(effective_vip(&state).await?))
}

pub fn cluster_vip_routes() -> Router<AppState> {
    Router::new()
        .route("/cluster/vip", get(get_vip).put(put_vip))
        .route("/cluster/masters/vip", get(agent_vip))
}
