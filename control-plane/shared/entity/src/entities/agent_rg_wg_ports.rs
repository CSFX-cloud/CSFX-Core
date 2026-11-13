use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_rg_wg_ports")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub agent_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub resource_group_id: Uuid,
    pub port: i32,
    pub updated_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agents::Entity",
        from = "Column::AgentId",
        to = "super::agents::Column::Id",
        on_delete = "Cascade"
    )]
    Agents,
    #[sea_orm(
        belongs_to = "super::resource_groups::Entity",
        from = "Column::ResourceGroupId",
        to = "super::resource_groups::Column::Id",
        on_delete = "Cascade"
    )]
    ResourceGroups,
}

impl ActiveModelBehavior for ActiveModel {}
