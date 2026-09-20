use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "resource_groups")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub internal_cidr: String,
    pub status: String,
    pub icon: String,
    pub color: String,
    pub pinned: bool,
    #[serde(skip_serializing)]
    pub icon_image: Option<Vec<u8>>,
    pub icon_image_mime: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::organization::Entity",
        from = "Column::OrganizationId",
        to = "super::organization::Column::Id",
        on_delete = "Cascade"
    )]
    Organization,
    #[sea_orm(has_many = "super::workloads::Entity")]
    Workloads,
    #[sea_orm(has_many = "super::volumes::Entity")]
    Volumes,
    #[sea_orm(has_many = "super::networks::Entity")]
    Networks,
    #[sea_orm(has_many = "super::workload_stacks::Entity")]
    WorkloadStacks,
    #[sea_orm(has_many = "super::resource_group_vpn_peers::Entity")]
    ResourceGroupVpnPeers,
    #[sea_orm(has_many = "super::buckets::Entity")]
    Buckets,
}

impl Related<super::organization::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Organization.def()
    }
}

impl Related<super::workloads::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Workloads.def()
    }
}

impl Related<super::volumes::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Volumes.def()
    }
}

impl Related<super::networks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Networks.def()
    }
}

impl Related<super::workload_stacks::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WorkloadStacks.def()
    }
}

impl Related<super::resource_group_vpn_peers::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ResourceGroupVpnPeers.def()
    }
}

impl Related<super::buckets::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Buckets.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
