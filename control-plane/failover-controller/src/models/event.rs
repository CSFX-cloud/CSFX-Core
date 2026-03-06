use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailoverEvent {
    pub id: Uuid,
    pub agent_id: Option<Uuid>,
    pub event_type: String,
    pub affected_workloads: Option<Vec<Uuid>>,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime<Utc>,
}
