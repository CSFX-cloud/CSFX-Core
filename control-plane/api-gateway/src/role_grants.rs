use axum::http::StatusCode;
use entity::{role, role_permission, user_organization, Role, RolePermission, UserOrganization};
use sea_orm::{ColumnTrait, DbErr, EntityTrait, PaginatorTrait, QueryFilter};
use std::collections::HashSet;
use uuid::Uuid;

const ADMIN_ROLE_NAME: &str = "Admin";

#[derive(Debug)]
pub enum GrantError {
    UnknownRole,
    NotMember,
    Forbidden,
    OwnRole,
    LastAdmin,
    Database(DbErr),
}

impl From<DbErr> for GrantError {
    fn from(cause: DbErr) -> Self {
        GrantError::Database(cause)
    }
}

impl GrantError {
    pub fn status(&self) -> StatusCode {
        match self {
            GrantError::UnknownRole | GrantError::NotMember => StatusCode::NOT_FOUND,
            GrantError::Forbidden => StatusCode::FORBIDDEN,
            GrantError::OwnRole => StatusCode::BAD_REQUEST,
            GrantError::LastAdmin => StatusCode::CONFLICT,
            GrantError::Database(cause) => {
                tracing::error!(error = %cause, "role grant check failed");
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            GrantError::UnknownRole => "role not found",
            GrantError::NotMember => "user is not a member of the organization",
            GrantError::Forbidden => "cannot grant permissions you do not hold",
            GrantError::OwnRole => "you cannot change your own role",
            GrantError::LastAdmin => "the last admin cannot be removed or demoted",
            GrantError::Database(_) => "database error",
        }
    }
}

async fn role_permission_ids(
    db: &impl sea_orm::ConnectionTrait,
    role_id: Uuid,
) -> Result<HashSet<Uuid>, DbErr> {
    Ok(RolePermission::find()
        .filter(role_permission::Column::RoleId.eq(role_id))
        .all(db)
        .await?
        .into_iter()
        .map(|grant| grant.permission_id)
        .collect())
}

async fn membership(
    db: &impl sea_orm::ConnectionTrait,
    user_id: Uuid,
    organization_id: Uuid,
) -> Result<Option<user_organization::Model>, DbErr> {
    UserOrganization::find()
        .filter(user_organization::Column::UserId.eq(user_id))
        .filter(user_organization::Column::OrganizationId.eq(organization_id))
        .one(db)
        .await
}

pub async fn user_permission_ids(
    db: &impl sea_orm::ConnectionTrait,
    user_id: Uuid,
    organization_id: Uuid,
) -> Result<HashSet<Uuid>, DbErr> {
    match membership(db, user_id, organization_id).await? {
        Some(member) => role_permission_ids(db, member.role_id).await,
        None => Ok(HashSet::new()),
    }
}

pub async fn ensure_permissions_held(
    db: &impl sea_orm::ConnectionTrait,
    actor_id: Uuid,
    organization_id: Uuid,
    wanted: &HashSet<Uuid>,
) -> Result<(), GrantError> {
    let held = user_permission_ids(db, actor_id, organization_id).await?;
    match wanted.is_subset(&held) {
        true => Ok(()),
        false => Err(GrantError::Forbidden),
    }
}

pub async fn ensure_role_grantable(
    db: &impl sea_orm::ConnectionTrait,
    actor_id: Uuid,
    organization_id: Uuid,
    role_id: Uuid,
) -> Result<role::Model, GrantError> {
    let target = Role::find_by_id(role_id)
        .filter(role::Column::OrganizationId.eq(organization_id))
        .one(db)
        .await?
        .ok_or(GrantError::UnknownRole)?;
    let wanted = role_permission_ids(db, role_id).await?;
    ensure_permissions_held(db, actor_id, organization_id, &wanted).await?;
    Ok(target)
}

pub async fn ensure_not_last_admin(
    db: &impl sea_orm::ConnectionTrait,
    organization_id: Uuid,
    user_id: Uuid,
) -> Result<(), GrantError> {
    let Some(admin_role) = Role::find()
        .filter(role::Column::OrganizationId.eq(organization_id))
        .filter(role::Column::Name.eq(ADMIN_ROLE_NAME))
        .one(db)
        .await?
    else {
        return Ok(());
    };
    let is_admin = membership(db, user_id, organization_id)
        .await?
        .is_some_and(|member| member.role_id == admin_role.id);
    if !is_admin {
        return Ok(());
    }
    let admins = UserOrganization::find()
        .filter(user_organization::Column::OrganizationId.eq(organization_id))
        .filter(user_organization::Column::RoleId.eq(admin_role.id))
        .count(db)
        .await?;
    match admins > 1 {
        true => Ok(()),
        false => Err(GrantError::LastAdmin),
    }
}

pub async fn ensure_role_change_allowed(
    db: &impl sea_orm::ConnectionTrait,
    actor_id: Uuid,
    organization_id: Uuid,
    target_user_id: Uuid,
    new_role_id: Uuid,
) -> Result<(), GrantError> {
    if actor_id == target_user_id {
        return Err(GrantError::OwnRole);
    }
    let current = membership(db, target_user_id, organization_id)
        .await?
        .ok_or(GrantError::NotMember)?;
    ensure_role_grantable(db, actor_id, organization_id, new_role_id).await?;
    if current.role_id == new_role_id {
        return Ok(());
    }
    let held_by_target = role_permission_ids(db, current.role_id).await?;
    ensure_permissions_held(db, actor_id, organization_id, &held_by_target).await?;
    ensure_not_last_admin(db, organization_id, target_user_id).await
}
