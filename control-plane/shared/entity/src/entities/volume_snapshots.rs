use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "volume_snapshots")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub volume_id: Uuid,
    pub name: String,
    pub status: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::volumes::Entity",
        from = "Column::VolumeId",
        to = "super::volumes::Column::Id"
    )]
    Volume,
}

impl Related<super::volumes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Volume.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
