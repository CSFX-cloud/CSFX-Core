use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json, Router,
};
use entity::{organization, role, user, Organization, Role, User};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{auth::middleware::AuthenticatedUser, rbac_service::RbacService, AppState};

pub fn routes() -> Router<AppState> {
    Router::new()
        // Single organization endpoints
        .route("/organization", axum::routing::get(get_organization))
        .route("/organization", axum::routing::put(update_organization))
        .route("/organization/roles", axum::routing::get(get_roles))
        // User management endpoints
        .route("/organization/users", axum::routing::get(list_users))
        .route("/organization/users", axum::routing::post(create_user))
        .route("/organization/users/:user_id", axum::routing::get(get_user))
        .route(
            "/organization/users/:user_id",
            axum::routing::put(update_user),
        )
        .route(
            "/organization/users/:user_id",
            axum::routing::delete(delete_user),
        )
        .route(
            "/organization/users/:user_id/role",
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
    pub joined_at: String,
}

// Get the single organization (always "Default Organization")
async fn get_organization(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<OrganizationResponse>, StatusCode> {
    let db = &state.db_conn;

    // Get the first (and only) organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(OrganizationResponse {
        id: org.id,
        name: org.name,
        description: org.description,
        created_at: org.created_at.to_string(),
        updated_at: org.updated_at.to_string(),
    }))
}

// Update the organization
async fn update_organization(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<UpdateOrganizationRequest>,
) -> Result<Json<OrganizationResponse>, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to update organization
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "organization", "update")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Update organization
    let mut active: organization::ActiveModel = org.into();
    active.name = ActiveValue::Set(req.name);
    active.description = ActiveValue::Set(req.description);
    active.updated_at = ActiveValue::Set(chrono::Utc::now().naive_utc());

    let updated = active
        .update(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(Json(OrganizationResponse {
        id: updated.id,
        name: updated.name,
        description: updated.description,
        created_at: updated.created_at.to_string(),
        updated_at: updated.updated_at.to_string(),
    }))
}

// Get all available roles
async fn get_roles(
    AuthenticatedUser(_claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<RoleResponse>>, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get all roles for this organization
    let roles = Role::find()
        .filter(role::Column::OrganizationId.eq(org.id))
        .all(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let response: Vec<RoleResponse> = roles
        .into_iter()
        .map(|r| RoleResponse {
            id: r.id,
            name: r.name,
            description: r.description,
            is_system_role: r.is_system_role,
        })
        .collect();

    Ok(Json(response))
}

// List all users in the organization
async fn list_users(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
) -> Result<Json<Vec<UserResponse>>, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to view users
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "users", "view")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get all members with their roles
    let members = rbac
        .get_organization_members(org.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut response = Vec::new();
    for (user_org, role) in members {
        let user = User::find_by_id(user_org.user_id)
            .one(db)
            .await
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
            .ok_or(StatusCode::NOT_FOUND)?;

        response.push(UserResponse {
            id: user.id,
            username: user.name,
            email: user.email,
            role_id: role.id,
            role_name: role.name,
            force_password_change: user.force_password_change,
            two_factor_enabled: user.two_factor_enabled,
            joined_at: user_org.joined_at.to_string(),
        });
    }

    Ok(Json(response))
}

// Get specific user
async fn get_user(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<Json<UserResponse>, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to view users
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "users", "view")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get user
    let user = User::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get user's role in this organization
    let role = rbac
        .get_user_role(user_id, org.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get user_organization for joined_at
    let user_org = entity::UserOrganization::find()
        .filter(entity::user_organization::Column::UserId.eq(user_id))
        .filter(entity::user_organization::Column::OrganizationId.eq(org.id))
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(UserResponse {
        id: user.id,
        username: user.name,
        email: user.email,
        role_id: role.id,
        role_name: role.name,
        force_password_change: user.force_password_change,
        two_factor_enabled: user.two_factor_enabled,
        joined_at: user_org.joined_at.to_string(),
    }))
}

// Create new user
async fn create_user(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Json(req): Json<CreateUserRequest>,
) -> Result<Json<UserResponse>, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to create users
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "users", "create")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Create user directly (not via AuthService as we need more control)
    let user_id = Uuid::new_v4();

    // Hash password
    let salt = crate::auth::crypto::generate_salt();
    let hashed_password = crate::auth::crypto::hash_password(&req.password, &salt)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let new_user = user::ActiveModel {
        id: ActiveValue::Set(user_id),
        name: ActiveValue::Set(req.username.clone()),
        password: ActiveValue::Set(hashed_password),
        salt: ActiveValue::Set(salt),
        email: ActiveValue::Set(req.email.clone()),
        two_factor_secret: ActiveValue::NotSet,
        two_factor_enabled: ActiveValue::Set(false),
        force_password_change: ActiveValue::Set(req.force_password_change),
    };

    let new_user = new_user
        .insert(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Assign role to user in organization
    rbac.assign_role_to_user(new_user.id, org.id, req.role_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get role name
    let role = Role::find_by_id(req.role_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(UserResponse {
        id: new_user.id,
        username: new_user.name,
        email: new_user.email,
        role_id: req.role_id,
        role_name: role.name,
        force_password_change: req.force_password_change,
        two_factor_enabled: false,
        joined_at: chrono::Utc::now().naive_utc().to_string(),
    }))
}

// Update user
async fn update_user(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRequest>,
) -> Result<Json<UserResponse>, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to update users
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "users", "update")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Get user
    let user = User::find_by_id(user_id)
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Update user
    let mut active: user::ActiveModel = user.into();
    if let Some(email) = req.email {
        active.email = ActiveValue::Set(Some(email));
    }
    if let Some(force_pwd) = req.force_password_change {
        active.force_password_change = ActiveValue::Set(force_pwd);
    }

    let updated = active
        .update(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Get user's role
    let role = rbac
        .get_user_role(user_id, org.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Get user_organization for joined_at
    let user_org = entity::UserOrganization::find()
        .filter(entity::user_organization::Column::UserId.eq(user_id))
        .filter(entity::user_organization::Column::OrganizationId.eq(org.id))
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    Ok(Json(UserResponse {
        id: updated.id,
        username: updated.name,
        email: updated.email,
        role_id: role.id,
        role_name: role.name,
        force_password_change: updated.force_password_change,
        two_factor_enabled: updated.two_factor_enabled,
        joined_at: user_org.joined_at.to_string(),
    }))
}

// Delete user
async fn delete_user(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
) -> Result<StatusCode, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to delete users
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "users", "delete")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Don't allow deleting yourself
    if user_id == claims.user_id {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Remove user from organization
    rbac.remove_user_from_organization(user_id, org.id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // Delete user
    User::delete_by_id(user_id)
        .exec(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::NO_CONTENT)
}

// Update user's role
async fn update_user_role(
    AuthenticatedUser(claims): AuthenticatedUser,
    State(state): State<AppState>,
    Path(user_id): Path<Uuid>,
    Json(req): Json<UpdateUserRoleRequest>,
) -> Result<StatusCode, StatusCode> {
    let db = &state.db_conn;

    // Get the organization
    let org = Organization::find()
        .one(db)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .ok_or(StatusCode::NOT_FOUND)?;

    // Check if user has permission to update users
    let rbac = RbacService::new(state.db_conn.clone());
    let has_perm = rbac
        .has_permission(claims.user_id, org.id, "users", "update")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !has_perm {
        return Err(StatusCode::FORBIDDEN);
    }

    // Update user's role
    rbac.assign_role_to_user(user_id, org.id, req.role_id)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(StatusCode::OK)
}
