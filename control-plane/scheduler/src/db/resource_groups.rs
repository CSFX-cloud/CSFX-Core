use entity::entities::resource_groups;
use sea_orm::{DatabaseConnection, EntityTrait};
use uuid::Uuid;

pub async fn internal_cidr(
    db: &DatabaseConnection,
    resource_group_id: Uuid,
) -> Result<Option<String>, sea_orm::DbErr> {
    Ok(resource_groups::Entity::find_by_id(resource_group_id)
        .one(db)
        .await?
        .map(|group| group.internal_cidr))
}
