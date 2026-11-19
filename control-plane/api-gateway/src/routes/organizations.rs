use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use entity::{
    organization, resource_group_vpn_peers, role, user, Organization, ResourceGroupVpnPeers, Role,
    User,
};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    auth::{
        middleware::AuthenticatedUser,
        rbac::{
            CanCreateUsers, CanDeleteUsers, CanUpdateOrganization, CanUpdateUsers, CanViewUsers,
        },
    },
    rbac_service::RbacService,
    role_grants::{
        ensure_not_last_admin, ensure_role_change_allowed, ensure_role_grantable,
        ensure_user_manageable,
    },
    AppState,
};

pub fn routes() -> Router<AppState> {
    Router::new()
        // Single organization endpoints
        .route("/organization", axum::routing::get(get_organization))
        .route("/organization", axum::routing::put(update_organization))
        .route("/organization/roles", axum::routing::get(get_roles))
        // User management endpoints
        .route("/organization/users", axum::routing::get(list_users))
        .route("/organization/users", axum::routing::post(create_user))
        .route(
            "/organization/users/{user_id}",
            axum::routing::get(get_user),
        )
        .route(
            "/organization/users/{user_id}",
            axum::routing::put(update_user),
        )
        .route(
            "/organization/users/{user_id}",
            axum::routing::delete(delete_user),
        )
        .route(
            "/organization/users/{user_id}/role",
            axum::routing::put(update_user_role),
        )
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateOrganizationRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizationResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RoleResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub is_system_role: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub email: Option<String>,
    pub password: String,
    pub role_id: Uuid,
    pub force_password_change: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserRequest {
    pub email: Option<String>,
    pub force_password_change: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserRoleRequest {
    pub role_id: Uuid,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UserResponse {
    pub id: Uuid,
    pub username: String,
    pub email: Option<String>,
    pub role_id: Uuid,
    pub role_name: String,
    pub force_password_change: bool,
    pub two_factor_enabled: bool,
    pub gravatar_email: Option<String>,
    pub auth_source: String,
    pub joined_at: String,
}

fn internal<E>(_: E) -> StatusCode {
    StatusCode::INTERNAL_SERVER_ERROR
}

fn organization_response(org: organization::Model) -> OrganizationResponse {
    OrganizationResponse {
        id: org.id,
        name: org.name,
        description: org.description,
        created_at: org.created_at.to_string(),
        updated_at: org.updated_at.to_string(),
    }
}

fn user_response(user: user::Model, role: role::Model, joined_at: String) -> UserResponse {
    UserResponse {
        id: user.id,
        username: user.name,
        email: user.email,
        role_id: role.id,
        role_name: role.name,
        force_password_change: user.force_password_change,
        two_factor_enabled: user.two_factor_enabled,
        gravatar_email: user.gravatar_email,
        auth_source: user.auth_source,
        joined_at,
    }
}

async fn load_user(state: &AppState, user_id: Uuid) -> Result<user::Model, StatusCode> {
    User::find_by_id(user_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)
}

async fn load_user_response(
    state: &AppState,
    user: user::Model,
) -> Result<UserResponse, StatusCode> {
    let org_id = state.default_org_id;
    let role = RbacService::new(state.db_conn.clone())
        .get_user_role(user.id, org_id)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    let user_org = entity::UserOrganization::find()
        .filter(entity::user_organization::Column::UserId.eq(user.id))
        .filter(entity::user_organization::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(user_response(user, role, user_org.joined_at.to_string()))
}

async fn get_organization(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<OrganizationResponse>, StatusCode> {
    let org = Organization::find_by_id(state.default_org_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(organization_response(org)))
}

async fn update_organization(
    CanUpdateOrganization(_claims): CanUpdateOrganization,
    State(state): State<AppState>,
    Json(req): Json<UpdateOrganizationRequest>,
) -> Result<Json<OrganizationResponse>, StatusCode> {
    let org = Organization::find_by_id(state.default_org_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let mut active: organization::ActiveModel = org.into();
    active.name = ActiveValue::Set(req.name);
    active.description = ActiveValue::Set(req.description);
    active.updated_at = ActiveValue::Set(chrono::Utc::now().naive_utc());

    let updated = active.update(&state.db_conn).await.map_err(internal)?;

    Ok(Json(organization_response(updated)))
}

async fn get_roles(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<RoleResponse>>, StatusCode> {
    let roles = Role::find()
        .filter(role::Column::OrganizationId.eq(state.default_org_id))
        .all(&state.db_conn)
        .await
        .map_err(internal)?;

    Ok(Json(
        roles
            .into_iter()
            .map(|r| RoleResponse {
                id: r.id,
                name: r.name,
                description: r.description,
                is_system_role: r.is_system_role,
            })
            .collect(),
    ))
}

async fn list_users(
    CanViewUsers(_claims): CanViewUsers,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserResponse>>, StatusCode> {
    let members = RbacService::new(state.db_conn.clone())
        .get_organization_members(state.default_org_id)
        .await
        .map_err(internal)?;

    let mut response = Vec::with_capacity(members.len());
    for (user_org, role) in members {
        let user = load_user(&state, user_org.user_id).await?;
        response.push(user_response(user, role, user_org.joined_at.to_string()));
    }

    Ok(Json(response))
}

async fn get_user(
    CanViewUsers(_claims): CanViewUsers,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserResponse>, StatusCode> {
    let user = load_user(&state, user_id).await?;
    Ok(Json(load_user_response(&state, user).await?))
}

async fn create_user(
    CanCreateUsers(claims): CanCreateUsers,
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, StatusCode> {
    let db = &state.db_conn;
    let org_id = state.default_org_id;

    ensure_role_grantable(db, claims.user_id, org_id, req.role_id)
        .await
        .map_err(|error| error.status())?;

    let salt = crate::auth::crypto::generate_salt();
    let hashed_password = crate::auth::crypto::hash_password(&req.password, &salt)
        .await
        .map_err(internal)?;

    let new_user = user::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        name: ActiveValue::Set(req.username),
        password: ActiveValue::Set(hashed_password),
        salt: ActiveValue::Set(salt),
        email: ActiveValue::Set(req.email),
        gravatar_email: ActiveValue::NotSet,
        two_factor_secret: ActiveValue::NotSet,
        two_factor_enabled: ActiveValue::Set(false),
        force_password_change: ActiveValue::Set(req.force_password_change),
        auth_source: ActiveValue::Set("local".to_string()),
    }
    .insert(db)
    .await
    .map_err(internal)?;

    RbacService::new(db.clone())
        .assign_role_to_user(new_user.id, org_id, req.role_id)
        .await
        .map_err(internal)?;

    let role = Role::find_by_id(req.role_id)
        .one(db)
        .await
        .map_err(internal)?
        .ok_or(StatusCode::NOT_FOUND)?;

    let joined_at = chrono::Utc::now().naive_utc().to_string();
    Ok(Json(user_response(new_user, role, joined_at)))
}

async fn update_user(
    CanUpdateUsers(claims): CanUpdateUsers,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, StatusCode> {
    let db = &state.db_conn;

    let user = load_user(&state, user_id).await?;

    ensure_user_manageable(db, claims.user_id, state.default_org_id, user_id)
        .await
        .map_err(|error| error.status())?;

    if let Some(email) = &req.email {
        let taken = User::find()
            .filter(user::Column::Email.eq(email))
            .filter(user::Column::Id.ne(user_id))
            .one(db)
            .await
            .map_err(internal)?
            .is_some();
        if taken {
            return Err(StatusCode::CONFLICT);
        }
    }

    let mut active: user::ActiveModel = user.into();
    if let Some(email) = req.email {
        active.email = ActiveValue::Set(Some(email));
    }
    if let Some(force_pwd) = req.force_password_change {
        active.force_password_change = ActiveValue::Set(force_pwd);
    }

    let updated = active.update(db).await.map_err(internal)?;

    Ok(Json(load_user_response(&state, updated).await?))
}

async fn delete_user(
    CanDeleteUsers(claims): CanDeleteUsers,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = &state.db_conn;
    let org_id = state.default_org_id;

    if user_id == claims.user_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    ensure_user_manageable(db, claims.user_id, org_id, user_id)
        .await
        .map_err(|error| error.status())?;

    ensure_not_last_admin(db, org_id, user_id)
        .await
        .map_err(|error| error.status())?;

    RbacService::new(db.clone())
        .remove_user_from_organization(user_id, org_id)
        .await
        .map_err(internal)?;

    crate::auth_service::AuthService::new(db.clone())
        .delete_users(&[user_id])
        .await
        .map_err(internal)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn update_user_role(
    CanUpdateUsers(claims): CanUpdateUsers,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRoleRequest>,
) -> Result<StatusCode, StatusCode> {
    let db = &state.db_conn;
    let org_id = state.default_org_id;
    let rbac = RbacService::new(db.clone());

    ensure_role_change_allowed(db, claims.user_id, org_id, user_id, req.role_id)
        .await
        .map_err(|error| error.status())?;

    rbac.assign_role_to_user(user_id, org_id, req.role_id)
        .await
        .map_err(internal)?;

    revoke_vpn_peers_without_access(&rbac, db, user_id, org_id)
        .await
        .map_err(internal)?;

    Ok(StatusCode::OK)
}

async fn revoke_vpn_peers_without_access(
    rbac: &RbacService,
    db: &sea_orm::DatabaseConnection,
    user_id: Uuid,
    organization_id: Uuid,
) -> Result<(), Box<dyn std::error::Error>> {
    if rbac
        .has_permission(user_id, organization_id, "resource_groups", "manage")
        .await?
    {
        return Ok(());
    }
    ResourceGroupVpnPeers::delete_many()
        .filter(resource_group_vpn_peers::Column::UserId.eq(user_id))
        .exec(db)
        .await?;
    Ok(())
}
