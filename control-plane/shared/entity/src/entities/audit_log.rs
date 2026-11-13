use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "audit_log")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub service: String,
    pub level: String,
    pub classification: String,
    pub message: String,
    pub agent_id: Option<Uuid>,
    pub workload_id: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub created_at: DateTimeWithTimeZone,
    pub kind: String,
    pub actor_user_id: Option<Uuid>,
    pub action: Option<String>,
    pub target_type: Option<String>,
    pub target_id: Option<String>,
    pub fields: Json,
    pub occurrences: i32,
    pub last_seen_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
