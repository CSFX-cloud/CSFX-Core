use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "vaults")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub resource_group_id: Uuid,
    pub name: String,
    pub sealed: bool,
    pub soft_delete_days: i32,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> Model {
        Model {
            id: Uuid::new_v4(),
            resource_group_id: Uuid::new_v4(),
            name: "production".to_string(),
            sealed: false,
            soft_delete_days: 7,
            created_at: chrono::NaiveDateTime::default(),
        }
    }

    #[test]
    fn model_round_trips_through_json() {
        let original = model();
        let raw = serde_json::to_string(&original).unwrap();
        assert_eq!(serde_json::from_str::<Model>(&raw).unwrap(), original);
    }

    #[test]
    fn active_model_conversion_preserves_fields() {
        let original = model();
        let active: ActiveModel = original.clone().into();
        assert_eq!(active.name.unwrap(), original.name);
        assert_eq!(
            active.resource_group_id.unwrap(),
            original.resource_group_id
        );
    }

    #[test]
    fn table_and_columns_are_named_after_the_schema() {
        assert_eq!(Entity.table_name(), "vaults");
        assert_eq!(Column::ResourceGroupId.to_string(), "resource_group_id");
    }
}
