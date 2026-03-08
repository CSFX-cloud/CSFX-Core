use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bootstrap_tokens")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub token: String,
    pub description: Option<String>,
    pub created_by: String,
    pub created_at: DateTime,
    pub expires_at: DateTime,
    pub max_uses: i32,
    pub use_count: i32,
    pub revoked: bool,
    pub revoked_at: Option<DateTime>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
