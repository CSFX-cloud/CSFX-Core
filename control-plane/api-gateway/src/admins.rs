use entity::{role, user, user_organization, Role, User, UserOrganization};
use sea_orm::{ColumnTrait, ConnectionTrait, DbErr, EntityTrait, QueryFilter};

pub async fn admin_users(db: &impl ConnectionTrait) -> Result<Vec<user::Model>, DbErr> {
    let admin_role_ids: Vec<_> = Role::find()
        .filter(role::Column::Name.eq("Admin"))
        .all(db)
        .await?
        .into_iter()
        .map(|role| role.id)
        .collect();
    let user_ids: Vec<_> = UserOrganization::find()
        .filter(user_organization::Column::RoleId.is_in(admin_role_ids))
        .all(db)
        .await?
        .into_iter()
        .map(|membership| membership.user_id)
        .collect();
    User::find()
        .filter(user::Column::Id.is_in(user_ids))
        .all(db)
        .await
}
