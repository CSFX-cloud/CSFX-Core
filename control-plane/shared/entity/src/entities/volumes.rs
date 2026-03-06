use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "volumes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub size_gb: i32,
    pub pool: String,
    pub image_name: String,
    pub status: String,
    pub attached_to_agent: Option<Uuid>,
    pub attached_to_workload: Option<Uuid>,
    pub mapped_device: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::agents::Entity",
        from = "Column::AttachedToAgent",
        to = "super::agents::Column::Id"
    )]
    Agent,
    #[sea_orm(has_many = "super::volume_snapshots::Entity")]
    Snapshots,
}

impl Related<super::agents::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Agent.def()
    }
}

impl Related<super::volume_snapshots::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Snapshots.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
