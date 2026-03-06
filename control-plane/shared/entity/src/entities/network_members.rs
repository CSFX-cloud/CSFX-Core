use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "network_members")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub network_id: Uuid,
    pub workload_id: Uuid,
    pub allocated_ip: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::networks::Entity",
        from = "Column::NetworkId",
        to = "super::networks::Column::Id"
    )]
    Network,
}

impl Related<super::networks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Network.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
