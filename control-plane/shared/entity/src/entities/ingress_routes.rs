use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "ingress_routes")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub workload_id: Uuid,
    pub gateway_id: Option<Uuid>,
    pub hostname: String,
    pub container_port: i32,
    pub tls: bool,
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
    #[sea_orm(
        belongs_to = "super::workloads::Entity",
        from = "Column::WorkloadId",
        to = "super::workloads::Column::Id",
        on_delete = "Cascade"
    )]
    Workload,
}

impl ActiveModelBehavior for ActiveModel {}
