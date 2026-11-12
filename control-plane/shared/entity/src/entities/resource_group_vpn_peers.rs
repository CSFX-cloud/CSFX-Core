use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "resource_group_vpn_peers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub user_id: Uuid,
    pub client_public_key: String,
    pub client_tunnel_ip: String,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::resource_groups::Entity",
        from = "Column::ResourceGroupId",
        to = "super::resource_groups::Column::Id",
        on_delete = "Cascade"
    )]
    ResourceGroups,
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::resource_groups::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ResourceGroups.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
