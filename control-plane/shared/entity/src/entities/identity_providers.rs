use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "identity_providers")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub organization_id: Uuid,
    pub slug: String,
    pub display_name: String,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret_encrypted: String,
    pub scopes: String,
    pub username_claim: String,
    pub email_claim: String,
    pub groups_claim: String,
    pub default_role_id: Option<Uuid>,
    pub auto_provision: bool,
    pub enabled: bool,
    pub created_at: DateTime,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
