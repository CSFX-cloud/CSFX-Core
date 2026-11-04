use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "site_env_vars")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub site_id: Uuid,
    pub name: String,
    pub value: Option<String>,
    pub vault_secret: Option<String>,
    pub scope: String,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::sites::Entity",
        from = "Column::SiteId",
        to = "super::sites::Column::Id",
        on_delete = "Cascade"
    )]
    Site,
}

impl ActiveModelBehavior for ActiveModel {}
