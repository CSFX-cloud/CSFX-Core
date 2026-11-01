use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "sites")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub name: String,
    pub mode: String,
    pub repo_url: String,
    pub production_branch: String,
    pub root_dir: String,
    pub framework: Option<String>,
    pub install_command: Option<String>,
    pub build_command: Option<String>,
    pub output_dir: Option<String>,
    pub start_command: Option<String>,
    pub container_port: Option<i32>,
    pub vault_id: Option<Uuid>,
    pub gateway_id: Option<Uuid>,
    pub git_token_secret: Option<String>,
    #[serde(skip_serializing)]
    pub webhook_secret: String,
    pub preview_enabled: bool,
    pub current_deployment_id: Option<Uuid>,
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
