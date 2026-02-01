use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq, Serialize, Deserialize)]
#[sea_orm(table_name = "user")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub password: String,
    pub salt: String,
    pub email: Option<String>,
    pub two_factor_secret: Option<String>,
    pub two_factor_enabled: bool,
    pub force_password_change: bool,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::expenses::Entity")]
    Expenses,
    #[sea_orm(has_many = "super::subscription::Entity")]
    Subscriptions,
}

impl Related<super::expenses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Expenses.def()
    }
}

impl Related<super::subscription::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Subscriptions.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
