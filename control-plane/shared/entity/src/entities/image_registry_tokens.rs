use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "image_registry_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub registry_id: Uuid,
    pub name: String,
    #[serde(skip_serializing)]
    pub token_hash: String,
    pub scope: String,
    pub created_at: DateTime,
    pub last_used_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::image_registries::Entity",
        from = "Column::RegistryId",
        to = "super::image_registries::Column::Id",
        on_delete = "Cascade"
    )]
    Registry,
}

impl ActiveModelBehavior for ActiveModel {}
