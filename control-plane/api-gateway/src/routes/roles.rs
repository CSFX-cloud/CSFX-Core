use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, put},
    Router,
};
use entity::{
    idp_group_mappings, permission, role, user_organization, IdpGroupMappings,
    Permission, Role, RolePermission, UserOrganization,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::{auth::rbac::CanManageRoles, rbac_service::RbacService, AppState};

type ApiError = (StatusCode, Json<Value>);

pub fn roles_routes() -> Router<AppState> {
    Router::new()
        .route("/permissions", get(list_permissions))
        .route("/roles", get(list_roles).post(create_role))
        .route("/roles/{id}", put(update_role).delete(delete_role))
}

#[derive(Serialize)]
struct RoleDetail {
    id: Uuid,
    name: String,
    description: Option<String>,
    is_system_role: bool,
    permission_ids: Vec<Uuid>,
}

#[derive(Deserialize)]
struct RolePayload {
    name: String,
    description: Option<String>,
    permission_ids: Vec<Uuid>,
}

fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({ "error": message })))
}

fn internal(context: &'static str) -> impl Fn(sea_orm::DbErr) -> ApiError {
    move |cause| {
        tracing::error!(error = %cause, "{context}");
        error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
    }
}

fn organization_id(state: &AppState) -> Result<Uuid, ApiError> {
    state
        .default_org_id
        .ok_or_else(|| error(StatusCode::INTERNAL_SERVER_ERROR, "organization not initialized"))
}

async fn load_role(
    db: &DatabaseConnection,
    id: Uuid,
    organization_id: Uuid,
) -> Result<role::Model, ApiError> {
    Role::find_by_id(id)
        .filter(role::Column::OrganizationId.eq(organization_id))
        .one(db)
        .await
        .map_err(internal("failed to load role"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "role not found"))
}

async fn ensure_permissions_exist(
    db: &DatabaseConnection,
    permission_ids: &[Uuid],
) -> Result<(), ApiError> {
    let found = Permission::find()
        .filter(permission::Column::Id.is_in(permission_ids.iter().copied()))
        .count(db)
        .await
        .map_err(internal("failed to verify permissions"))?;
    match found as usize == permission_ids.len() {
        true => Ok(()),
        false => Err(error(StatusCode::BAD_REQUEST, "unknown permission id")),
    }
}

async fn ensure_unique_name(
    db: &DatabaseConnection,
    organization_id: Uuid,
    name: &str,
    except: Option<Uuid>,
) -> Result<(), ApiError> {
    let clash = Role::find()
        .filter(role::Column::OrganizationId.eq(organization_id))
        .filter(role::Column::Name.eq(name))
        .one(db)
        .await
        .map_err(internal("failed to check role name"))?;
    match clash {
        Some(existing) if Some(existing.id) != except => {
            Err(error(StatusCode::CONFLICT, "role name already in use"))
        }
        _ => Ok(()),
    }
}

async fn validated_payload(
    db: &DatabaseConnection,
    mut payload: RolePayload,
) -> Result<RolePayload, ApiError> {
    payload.name = payload.name.trim().to_string();
    if payload.name.is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "name is required"));
    }
    let unique: HashSet<Uuid> = payload.permission_ids.drain(..).collect();
    payload.permission_ids = unique.into_iter().collect();
    ensure_permissions_exist(db, &payload.permission_ids).await?;
    Ok(payload)
}

async fn list_permissions(
    CanManageRoles(_claims): CanManageRoles,
    State(state): State<AppState>,
) -> Result<Json<Vec<permission::Model>>, ApiError> {
    let permissions = Permission::find()
        .order_by_asc(permission::Column::Name)
        .all(&state.db_conn)
        .await
        .map_err(internal("failed to list permissions"))?;
    Ok(Json(permissions))
}

async fn list_roles(
    CanManageRoles(_claims): CanManageRoles,
    State(state): State<AppState>,
) -> Result<Json<Vec<RoleDetail>>, ApiError> {
    let organization_id = organization_id(&state)?;
    let roles = Role::find()
        .filter(role::Column::OrganizationId.eq(organization_id))
        .order_by_asc(role::Column::Name)
        .all(&state.db_conn)
        .await
        .map_err(internal("failed to list roles"))?;
    let grants = RolePermission::find()
        .all(&state.db_conn)
        .await
        .map_err(internal("failed to list role permissions"))?;
    let mut by_role: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for grant in grants {
        by_role.entry(grant.role_id).or_default().push(grant.permission_id);
    }
    Ok(Json(
        roles
            .into_iter()
            .map(|r| RoleDetail {
                permission_ids: by_role.remove(&r.id).unwrap_or_default(),
                id: r.id,
                name: r.name,
                description: r.description,
                is_system_role: r.is_system_role,
            })
            .collect(),
    ))
}

async fn create_role(
    CanManageRoles(claims): CanManageRoles,
    State(state): State<AppState>,
    Json(payload): Json<RolePayload>,
) -> Result<(StatusCode, Json<RoleDetail>), ApiError> {
    let organization_id = organization_id(&state)?;
    let payload = validated_payload(&state.db_conn, payload).await?;
    ensure_unique_name(&state.db_conn, organization_id, &payload.name, None).await?;
    let rbac = RbacService::new(state.db_conn.clone());
    let created = rbac
        .create_role(organization_id, payload.name, payload.description, false)
        .await
        .map_err(|cause| internal_rbac(cause.to_string()))?;
    rbac.assign_permissions_to_role(created.id, payload.permission_ids.clone())
        .await
        .map_err(|cause| internal_rbac(cause.to_string()))?;
    tracing::info!(role_id = %created.id, actor = %claims.user_id, "role created");
    Ok((StatusCode::CREATED, Json(detail(created, payload.permission_ids))))
}

fn internal_rbac(cause: String) -> ApiError {
    tracing::error!(error = %cause, "role storage failed");
    error(StatusCode::INTERNAL_SERVER_ERROR, "role storage failed")
}

fn detail(model: role::Model, permission_ids: Vec<Uuid>) -> RoleDetail {
    RoleDetail {
        id: model.id,
        name: model.name,
        description: model.description,
        is_system_role: model.is_system_role,
        permission_ids,
    }
}

async fn update_role(
    CanManageRoles(claims): CanManageRoles,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<RolePayload>,
) -> Result<Json<RoleDetail>, ApiError> {
    let organization_id = organization_id(&state)?;
    let existing = load_role(&state.db_conn, id, organization_id).await?;
    if existing.is_system_role {
        return Err(error(StatusCode::CONFLICT, "system roles cannot be modified"));
    }
    let payload = validated_payload(&state.db_conn, payload).await?;
    ensure_unique_name(&state.db_conn, organization_id, &payload.name, Some(id)).await?;
    let mut active: role::ActiveModel = existing.into();
    active.name = Set(payload.name);
    active.description = Set(payload.description);
    let updated = active
        .update(&state.db_conn)
        .await
        .map_err(internal("failed to update role"))?;
    RbacService::new(state.db_conn.clone())
        .assign_permissions_to_role(id, payload.permission_ids.clone())
        .await
        .map_err(|cause| internal_rbac(cause.to_string()))?;
    tracing::info!(role_id = %id, actor = %claims.user_id, "role updated");
    Ok(Json(detail(updated, payload.permission_ids)))
}

async fn role_in_use(db: &DatabaseConnection, id: Uuid) -> Result<bool, ApiError> {
    let members = UserOrganization::find()
        .filter(user_organization::Column::RoleId.eq(id))
        .count(db)
        .await
        .map_err(internal("failed to count role members"))?;
    let mappings = IdpGroupMappings::find()
        .filter(idp_group_mappings::Column::RoleId.eq(id))
        .count(db)
        .await
        .map_err(internal("failed to count role mappings"))?;
    Ok(members + mappings > 0)
}

async fn delete_role(
    CanManageRoles(claims): CanManageRoles,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let existing = load_role(&state.db_conn, id, organization_id(&state)?).await?;
    if existing.is_system_role {
        return Err(error(StatusCode::CONFLICT, "system roles cannot be deleted"));
    }
    if role_in_use(&state.db_conn, id).await? {
        return Err(error(StatusCode::CONFLICT, "role is still assigned or mapped"));
    }
    Role::delete_by_id(id)
        .exec(&state.db_conn)
        .await
        .map_err(internal("failed to delete role"))?;
    tracing::info!(role_id = %id, actor = %claims.user_id, "role deleted");
    Ok(StatusCode::NO_CONTENT)
}
