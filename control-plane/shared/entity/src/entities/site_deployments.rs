use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "site_deployments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub site_id: Uuid,
    pub commit_sha: String,
    pub commit_message: String,
    pub branch: String,
    pub pull_request: Option<i32>,
    pub environment: String,
    pub status: String,
    pub artifact_key: Option<String>,
    pub image_ref: Option<String>,
    pub workload_id: Option<Uuid>,
    pub build_log: String,
    pub duration_ms: Option<i64>,
    pub created_at: DateTime,
    pub finished_at: Option<DateTime>,
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
