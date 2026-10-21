use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "dns_records")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub resource_group_id: Uuid,
    #[sea_orm(unique)]
    pub name: String,
    pub record_type: String,
    pub value: String,
    pub ttl: i32,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::resource_groups::Entity",
        from = "Column::ResourceGroupId",
        to = "super::resource_groups::Column::Id",
        on_delete = "Cascade"
    )]
    ResourceGroup,
}

impl ActiveModelBehavior for ActiveModel {}
