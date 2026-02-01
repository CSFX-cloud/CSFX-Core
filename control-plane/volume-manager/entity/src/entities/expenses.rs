use sea_orm::entity::prelude::*;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use utoipa::ToSchema;

// Custom serialization for Decimal to string
fn serialize_decimal<S>(decimal: &Decimal, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_str(&decimal.to_string())
}

// Custom deserialization for Decimal from string
fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse::<Decimal>().map_err(serde::de::Error::custom)
}

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize, ToSchema)]
#[sea_orm(table_name = "expenses")]
pub struct Model {
    #[sea_orm(primary_key)]
    /// Unique identifier for the expense
    pub id: Uuid,
    /// User ID who owns this expense
    pub user_id: Uuid,
    /// Description of the expense
    pub description: String,
    /// Amount of the expense
    #[serde(
        serialize_with = "serialize_decimal",
        deserialize_with = "deserialize_decimal"
    )]
    pub amount: Decimal,
    /// Date and time of the expense
    pub date: DateTime,
    /// Category of the expense
    pub category: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
