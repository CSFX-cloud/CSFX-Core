use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post, put},
    Router,
};
use entity::entities::resource_groups;
use entity::Organization;
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthenticatedUser;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateResourceGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateResourceGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub location: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceGroupResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub organization_id: Uuid,
    pub created_by: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Option<serde_json::Value>,
    pub location: Option<String>,
}

impl From<resource_groups::Model> for ResourceGroupResponse {
    fn from(model: resource_groups::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
            organization_id: model.organization_id,
            created_by: model.created_by,
            created_at: model.created_at.to_string(),
            updated_at: model.updated_at.to_string(),
            tags: model.tags,
            location: model.location,
        }
    }
}

/// List all resource groups for the user's organization
async fn list_resource_groups(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the organization
    let org = match Organization::find().one(db).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Organization not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get organization: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve organization"
                })),
            )
                .into_response();
        }
    };

    match resource_groups::Entity::find()
        .filter(resource_groups::Column::OrganizationId.eq(org.id))
        .order_by_desc(resource_groups::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(resource_groups) => {
            let response: Vec<ResourceGroupResponse> =
                resource_groups.into_iter().map(Into::into).collect();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to list resource groups: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource groups"
                })),
            )
                .into_response()
        }
    }
}

/// Get a specific resource group by ID
async fn get_resource_group(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the organization
    let org = match Organization::find().one(db).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Organization not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get organization: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve organization"
                })),
            )
                .into_response();
        }
    };

    match resource_groups::Entity::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org.id))
        .one(db)
        .await
    {
        Ok(Some(resource_group)) => {
            let response: ResourceGroupResponse = resource_group.into();
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Resource group not found"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get resource group: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource group"
                })),
            )
                .into_response()
        }
    }
}

/// Create a new resource group
async fn create_resource_group(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateResourceGroupRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the organization
    let org = match Organization::find().one(db).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Organization not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get organization: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve organization"
                })),
            )
                .into_response();
        }
    };

    // Check if a resource group with the same name already exists in the organization
    match resource_groups::Entity::find()
        .filter(resource_groups::Column::OrganizationId.eq(org.id))
        .filter(resource_groups::Column::Name.eq(&payload.name))
        .one(db)
        .await
    {
        Ok(Some(_)) => {
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": "A resource group with this name already exists in your organization"
                })),
            )
                .into_response()
        }
        Ok(None) => {}
        Err(e) => {
            tracing::error!("Failed to check for existing resource group: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to create resource group"
                })),
            )
                .into_response();
        }
    }

    let now = chrono::Utc::now().naive_utc();
    let new_resource_group = resource_groups::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        name: ActiveValue::Set(payload.name),
        description: ActiveValue::Set(payload.description),
        organization_id: ActiveValue::Set(org.id),
        created_by: ActiveValue::Set(Some(user.user_id)),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
        tags: ActiveValue::Set(payload.tags),
        location: ActiveValue::Set(payload.location),
    };

    match new_resource_group.insert(db).await {
        Ok(resource_group) => {
            let response: ResourceGroupResponse = resource_group.into();
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create resource group: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to create resource group"
                })),
            )
                .into_response()
        }
    }
}

/// Update an existing resource group
async fn update_resource_group(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateResourceGroupRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the organization
    let org = match Organization::find().one(db).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Organization not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get organization: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve organization"
                })),
            )
                .into_response();
        }
    };

    // Find the resource group
    let resource_group = match resource_groups::Entity::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org.id))
        .one(db)
        .await
    {
        Ok(Some(rg)) => rg,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource group not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to find resource group: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to update resource group"
                })),
            )
                .into_response();
        }
    };

    // Check if name is being updated and if it conflicts with another resource group
    if let Some(ref new_name) = payload.name {
        if new_name != &resource_group.name {
            match resource_groups::Entity::find()
                .filter(resource_groups::Column::OrganizationId.eq(org.id))
                .filter(resource_groups::Column::Name.eq(new_name))
                .filter(resource_groups::Column::Id.ne(id))
                .one(db)
                .await
            {
                Ok(Some(_)) => {
                    return (
                        StatusCode::CONFLICT,
                        Json(serde_json::json!({
                            "error": "A resource group with this name already exists in your organization"
                        })),
                    )
                        .into_response()
                }
                Ok(None) => {}
                Err(e) => {
                    tracing::error!("Failed to check for existing resource group: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "Failed to update resource group"
                        })),
                    )
                        .into_response();
                }
            }
        }
    }

    let mut active_model: resource_groups::ActiveModel = resource_group.into();

    if let Some(name) = payload.name {
        active_model.name = ActiveValue::Set(name);
    }
    if let Some(description) = payload.description {
        active_model.description = ActiveValue::Set(Some(description));
    }
    if let Some(location) = payload.location {
        active_model.location = ActiveValue::Set(Some(location));
    }
    if let Some(tags) = payload.tags {
        active_model.tags = ActiveValue::Set(Some(tags));
    }
    active_model.updated_at = ActiveValue::Set(chrono::Utc::now().naive_utc());

    match active_model.update(db).await {
        Ok(updated_resource_group) => {
            let response: ResourceGroupResponse = updated_resource_group.into();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update resource group: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to update resource group"
                })),
            )
                .into_response()
        }
    }
}

/// Delete a resource group
async fn delete_resource_group(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the organization
    let org = match Organization::find().one(db).await {
        Ok(Some(o)) => o,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Organization not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get organization: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve organization"
                })),
            )
                .into_response();
        }
    };

    // Find the resource group
    let resource_group = match resource_groups::Entity::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org.id))
        .one(db)
        .await
    {
        Ok(Some(rg)) => rg,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource group not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to find resource group: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to delete resource group"
                })),
            )
                .into_response();
        }
    };

    let active_model: resource_groups::ActiveModel = resource_group.into();

    match active_model.delete(db).await {
        Ok(_) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "message": "Resource group deleted successfully"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to delete resource group: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to delete resource group"
                })),
            )
                .into_response()
        }
    }
}

pub fn resource_groups_routes() -> Router<AppState> {
    Router::new()
        .route("/resource-groups", get(list_resource_groups))
        .route("/resource-groups/:id", get(get_resource_group))
        .route("/resource-groups", post(create_resource_group))
        .route("/resource-groups/:id", put(update_resource_group))
        .route("/resource-groups/:id", delete(delete_resource_group))
}
