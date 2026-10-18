use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use chrono::{DateTime, TimeZone, Utc};
use entity::entities::logs;
use sea_orm::{ActiveValue::Set, EntityTrait};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{auth::agent::AgentApiKey, AppState};

const MAX_ENTRIES: usize = 200;
const MAX_MESSAGE_CHARS: usize = 4000;
const AGENT_SERVICE: &str = "agent";

#[derive(Debug, Deserialize)]
pub struct AgentLogEntry {
    pub level: String,
    pub unit: String,
    pub message: String,
    pub timestamp_micros: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AgentLogBatch {
    pub entries: Vec<AgentLogEntry>,
}

fn normalized_level(level: &str) -> &'static str {
    match level {
        "ERROR" => "ERROR",
        _ => "WARN",
    }
}

fn entry_time(entry: &AgentLogEntry) -> DateTime<Utc> {
    entry
        .timestamp_micros
        .and_then(|micros| Utc.timestamp_micros(micros).single())
        .unwrap_or_else(Utc::now)
}

fn to_model(agent_id: Uuid, entry: AgentLogEntry) -> logs::ActiveModel {
    let at = entry_time(&entry);
    logs::ActiveModel {
        id: Set(Uuid::new_v4()),
        service: Set(AGENT_SERVICE.to_string()),
        level: Set(normalized_level(&entry.level).to_string()),
        classification: Set("system".to_string()),
        message: Set(entry.message.chars().take(MAX_MESSAGE_CHARS).collect()),
        agent_id: Set(Some(agent_id)),
        workload_id: Set(None),
        organization_id: Set(None),
        created_at: Set(at.into()),
        kind: Set("system".to_string()),
        actor_user_id: Set(None),
        action: Set(None),
        target_type: Set(None),
        target_id: Set(None),
        fields: Set(json!({ "unit": entry.unit })),
        occurrences: Set(1),
        last_seen_at: Set(at.into()),
    }
}

pub async fn receive_agent_logs(
    agent: AgentApiKey,
    State(state): State<AppState>,
    Json(batch): Json<AgentLogBatch>,
) -> Result<StatusCode, StatusCode> {
    if batch.entries.is_empty() {
        return Ok(StatusCode::NO_CONTENT);
    }
    if batch.entries.len() > MAX_ENTRIES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let models: Vec<logs::ActiveModel> = batch
        .entries
        .into_iter()
        .map(|entry| to_model(agent.agent_id, entry))
        .collect();

    logs::Entity::insert_many(models)
        .exec(&state.db_conn)
        .await
        .map_err(|error| {
            tracing::error!(error = %error, agent_id = %agent.agent_id, "failed to store agent logs");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    Ok(StatusCode::NO_CONTENT)
}

pub fn agent_logs_routes() -> Router<AppState> {
    Router::new().route("/agents/self/logs", post(receive_agent_logs))
}
