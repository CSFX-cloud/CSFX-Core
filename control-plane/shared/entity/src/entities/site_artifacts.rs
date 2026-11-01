use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "site_artifacts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub deployment_id: Uuid,
    pub content: Vec<u8>,
    pub size_bytes: i64,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::site_deployments::Entity",
        from = "Column::DeploymentId",
        to = "super::site_deployments::Column::Id",
        on_delete = "Cascade"
    )]
    Deployment,
}

impl ActiveModelBehavior for ActiveModel {}
