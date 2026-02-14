use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "registry_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub token: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime,
    pub expires_at: DateTime,
    pub used_at: Option<DateTime>,
    pub used_by_agent_id: Option<Uuid>,
    pub is_used: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agents::Entity",
        from = "Column::UsedByAgentId",
        to = "super::agents::Column::Id",
        on_update = "NoAction",
        on_delete = "SetNull"
    )]
    Agent,
}

impl Related<super::agents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Agent.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
