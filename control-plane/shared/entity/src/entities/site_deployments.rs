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
    #[serde(skip_serializing)]
    pub build_env: Option<Json>,
    pub build_phases: Option<Json>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_deployments_never_carry_the_build_env() {
        let model = Model {
            id: Uuid::nil(),
            site_id: Uuid::nil(),
            commit_sha: String::new(),
            commit_message: String::new(),
            branch: "main".to_string(),
            pull_request: None,
            environment: "production".to_string(),
            status: "building".to_string(),
            artifact_key: None,
            image_ref: None,
            workload_id: None,
            build_log: String::new(),
            build_env: Some(serde_json::json!({ "API_KEY": "secret" })),
            build_phases: None,
            duration_ms: None,
            created_at: chrono::Utc::now().naive_utc(),
            finished_at: None,
        };
        let serialized = serde_json::to_string(&model).unwrap();
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("build_env"));
    }
}
