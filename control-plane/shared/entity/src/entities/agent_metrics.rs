use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_metrics")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub agent_id: Uuid,
    pub timestamp: DateTime,

    // CPU
    pub cpu_model: Option<String>,
    pub cpu_cores: Option<i32>,
    pub cpu_threads: Option<i32>,
    pub cpu_usage_percent: Option<f32>,

    // Memory
    pub memory_total_bytes: Option<i64>,
    pub memory_used_bytes: Option<i64>,
    pub memory_usage_percent: Option<f32>,

    // Disk
    pub disk_total_bytes: Option<i64>,
    pub disk_used_bytes: Option<i64>,
    pub disk_usage_percent: Option<f32>,

    // Network
    pub network_rx_bytes: Option<i64>,
    pub network_tx_bytes: Option<i64>,

    // System
    pub os_name: Option<String>,
    pub os_version: Option<String>,
    pub kernel_version: Option<String>,
    pub hostname: Option<String>,
    pub uptime_seconds: Option<i64>,

    // Custom metrics (JSON)
    pub custom_metrics: Option<Json>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agents::Entity",
        from = "Column::AgentId",
        to = "super::agents::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Agent,
}

impl Related<super::agents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Agent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
