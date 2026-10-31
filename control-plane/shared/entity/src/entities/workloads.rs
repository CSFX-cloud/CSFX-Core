use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "workloads")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub image: String,
    pub cpu_millicores: i32,
    pub memory_bytes: i64,
    pub disk_bytes: i64,
    pub cpu_limit_millicores: Option<i32>,
    pub memory_limit_bytes: Option<i64>,
    pub spread_key: Option<String>,
    pub guest_ip: Option<String>,
    pub auto_resources: bool,
    pub cpu_usage_peak_percent: Option<f64>,
    pub memory_usage_peak_bytes: Option<i64>,
    pub env_vars: Option<Json>,
    pub ports: Option<Json>,
    pub volume_mounts: Option<Json>,
    pub status: String,
    pub assigned_agent_id: Option<Uuid>,
    pub container_id: Option<String>,
    pub created_by: Option<Uuid>,
    pub organization_id: Option<Uuid>,
    pub resource_group_id: Option<Uuid>,
    pub stack_id: Option<Uuid>,
    pub service_name: Option<String>,
    pub restart_policy: String,
    pub max_restarts: Option<i32>,
    pub restart_count: i32,
    pub runtime_class: String,
    pub desired_state: String,
    pub restart_requested: bool,
    pub cpu_usage_percent: Option<f64>,
    pub memory_usage_bytes: Option<i64>,
    pub network_rx_bytes: Option<i64>,
    pub network_tx_bytes: Option<i64>,
    pub stats_updated_at: Option<DateTime>,
    pub created_at: DateTime,
    pub updated_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agents::Entity",
        from = "Column::AssignedAgentId",
        to = "super::agents::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Agent,
    #[sea_orm(
        belongs_to = "super::resource_groups::Entity",
        from = "Column::ResourceGroupId",
        to = "super::resource_groups::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    ResourceGroup,
    #[sea_orm(
        belongs_to = "super::workload_stacks::Entity",
        from = "Column::StackId",
        to = "super::workload_stacks::Column::Id",
        on_update = "NoAction",
        on_delete = "Cascade"
    )]
    Stack,
}

impl Related<super::agents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Agent.def()
    }
}

impl Related<super::resource_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ResourceGroup.def()
    }
}

impl Related<super::workload_stacks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Stack.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
