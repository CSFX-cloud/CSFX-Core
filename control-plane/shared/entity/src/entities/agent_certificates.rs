use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "agent_certificates")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub agent_id: Uuid,
    pub serial_number: i64,
    pub certificate_pem: String,
    pub public_key_pem: String,
    pub issued_at: DateTime,
    pub expires_at: DateTime,
    pub is_active: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agents::Entity",
        from = "Column::AgentId",
        to = "super::agents::Column::Id",
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
