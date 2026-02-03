use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
    routing::{delete, get, post, put},
    Router,
};
use entity::entities::{docker_resources, resource_groups};
use entity::{DockerResources, Organization, ResourceGroups};
use sea_orm::{ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::auth::middleware::AuthenticatedUser;
use crate::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateResourceRequest {
    pub name: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub resource_group_id: Uuid,
    pub configuration: Option<serde_json::Value>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateResourceRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub configuration: Option<serde_json::Value>,
    pub status: Option<String>,
    pub tags: Option<serde_json::Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceActionRequest {
    pub action: String, // "start", "stop", "restart"
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DeployContainerRequest {
    pub name: String,
    pub image: String,
    pub resource_group_id: Uuid,
    pub description: Option<String>,
    pub ports: Option<Vec<serde_json::Value>>,
    pub environment: Option<serde_json::Value>,
    pub volumes: Option<Vec<serde_json::Value>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ResourceResponse {
    pub id: Uuid,
    pub name: String,
    pub resource_type: String,
    pub description: Option<String>,
    pub resource_group_id: Uuid,
    pub resource_group_name: String,
    pub configuration: Option<serde_json::Value>,
    pub status: String,
    pub created_by: Option<Uuid>,
    pub created_at: String,
    pub updated_at: String,
    pub tags: Option<serde_json::Value>,
    pub container_id: Option<String>,
    pub stack_name: Option<String>,
}

/// List all resources across all resource groups
async fn list_resources(
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

    // Get all resource groups for this org
    let resource_group_ids: Vec<Uuid> = match ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org.id))
        .all(db)
        .await
    {
        Ok(groups) => groups.into_iter().map(|g| g.id).collect(),
        Err(e) => {
            tracing::error!("Failed to get resource groups: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource groups"
                })),
            )
                .into_response();
        }
    };

    // Get all resources for these resource groups
    match DockerResources::find()
        .filter(docker_resources::Column::ResourceGroupId.is_in(resource_group_ids))
        .order_by_desc(docker_resources::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(resources) => {
            let mut response_resources = Vec::new();

            // Fetch resource group names
            for resource in resources {
                if let Ok(Some(rg)) = ResourceGroups::find_by_id(resource.resource_group_id)
                    .one(db)
                    .await
                {
                    response_resources.push(ResourceResponse {
                        id: resource.id,
                        name: resource.name,
                        resource_type: resource.resource_type,
                        description: resource.description,
                        resource_group_id: resource.resource_group_id,
                        resource_group_name: rg.name,
                        configuration: resource.configuration,
                        status: resource.status,
                        created_by: resource.created_by,
                        created_at: resource.created_at.to_string(),
                        updated_at: resource.updated_at.to_string(),
                        tags: resource.tags,
                        container_id: resource.container_id,
                        stack_name: resource.stack_name,
                    });
                }
            }

            (StatusCode::OK, Json(response_resources)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get resources: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resources"
                })),
            )
                .into_response()
        }
    }
}

/// List resources for a specific resource group
async fn list_resources_by_group(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(resource_group_id): Path<Uuid>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Verify resource group exists and belongs to user's org
    let _rg = match ResourceGroups::find_by_id(resource_group_id).one(db).await {
        Ok(Some(rg)) => rg,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource group not found"
                })),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get resource group: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource group"
                })),
            )
                .into_response();
        }
    };

    match DockerResources::find()
        .filter(docker_resources::Column::ResourceGroupId.eq(resource_group_id))
        .order_by_desc(docker_resources::Column::CreatedAt)
        .all(db)
        .await
    {
        Ok(resources) => {
            let response_resources: Vec<ResourceResponse> = resources
                .into_iter()
                .map(|r| ResourceResponse {
                    id: r.id,
                    name: r.name.clone(),
                    resource_type: r.resource_type.clone(),
                    description: r.description.clone(),
                    resource_group_id: r.resource_group_id,
                    resource_group_name: _rg.name.clone(),
                    configuration: r.configuration.clone(),
                    status: r.status.clone(),
                    created_by: r.created_by,
                    created_at: r.created_at.to_string(),
                    updated_at: r.updated_at.to_string(),
                    tags: r.tags.clone(),
                    container_id: r.container_id.clone(),
                    stack_name: r.stack_name.clone(),
                })
                .collect();

            (StatusCode::OK, Json(response_resources)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get resources: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resources"
                })),
            )
                .into_response()
        }
    }
}

/// Get a specific resource by ID
async fn get_resource(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    match DockerResources::find_by_id(id).one(db).await {
        Ok(Some(resource)) => {
            let container_id_opt = resource.container_id.clone();
            let current_status = resource.status.clone();

            // Check Docker status if available
            let synced_resource = if let (Some(docker), Some(container_id)) =
                (&state.docker, &container_id_opt)
            {
                match docker.inspect_container(container_id).await {
                    Ok(container_info) => {
                        // Update status from Docker
                        let docker_status = match container_info.state.as_str() {
                            "running" => "running",
                            "exited" | "dead" => "stopped",
                            _ => "error",
                        };

                        // Update in database if changed
                        if current_status != docker_status {
                            let mut resource_active: docker_resources::ActiveModel =
                                resource.into();
                            resource_active.status = ActiveValue::Set(docker_status.to_string());
                            resource_active.updated_at =
                                ActiveValue::Set(chrono::Utc::now().naive_utc());

                            match resource_active.update(db).await {
                                Ok(updated) => {
                                    tracing::info!(
                                        "Synced status for container {} from Docker: {}",
                                        container_id,
                                        docker_status
                                    );
                                    updated
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to update resource status: {}", e);
                                    return (
                                        StatusCode::INTERNAL_SERVER_ERROR,
                                        Json(serde_json::json!({
                                            "error": "Failed to sync container status"
                                        })),
                                    )
                                        .into_response();
                                }
                            }
                        } else {
                            resource
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to get container info from Docker: {}", e);
                        resource
                    }
                }
            } else {
                resource
            };

            // Get resource group name
            let rg_name = if let Ok(Some(rg)) =
                ResourceGroups::find_by_id(synced_resource.resource_group_id)
                    .one(db)
                    .await
            {
                rg.name
            } else {
                "Unknown".to_string()
            };

            let response = ResourceResponse {
                id: synced_resource.id,
                name: synced_resource.name,
                resource_type: synced_resource.resource_type,
                description: synced_resource.description,
                resource_group_id: synced_resource.resource_group_id,
                resource_group_name: rg_name,
                configuration: synced_resource.configuration,
                status: synced_resource.status,
                created_by: synced_resource.created_by,
                created_at: synced_resource.created_at.to_string(),
                updated_at: synced_resource.updated_at.to_string(),
                tags: synced_resource.tags,
                container_id: synced_resource.container_id,
                stack_name: synced_resource.stack_name,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Resource not found"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get resource: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource"
                })),
            )
                .into_response()
        }
    }
}

/// Create a new resource
async fn create_resource(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<CreateResourceRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Verify resource group exists
    let rg = match ResourceGroups::find_by_id(payload.resource_group_id)
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
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get resource group: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource group"
                })),
            )
                .into_response();
        }
    };

    let now = chrono::Utc::now().naive_utc();

    // For docker-container type, create the container in Docker if service is available
    let (initial_status, container_id_opt) = if payload.resource_type == "docker-container" {
        if let Some(docker) = &state.docker {
            // Get image from configuration
            if let Some(image) = payload
                .configuration
                .as_ref()
                .and_then(|c| c.get("image"))
                .and_then(|i| i.as_str())
            {
                tracing::info!("Creating Docker container with image: {}", image);

                // Pull image first
                match docker.pull_image(image).await {
                    Ok(_) => tracing::info!("Successfully pulled image: {}", image),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to pull image {}: {}. Trying to use local image.",
                            image,
                            e
                        );
                    }
                }

                // Create and start container
                let container_name = format!(
                    "{}-{}",
                    payload.name.to_lowercase().replace(" ", "-"),
                    &Uuid::new_v4().to_string()[..8]
                );

                match docker
                    .create_and_start_container(
                        &container_name,
                        image,
                        payload
                            .configuration
                            .as_ref()
                            .unwrap_or(&serde_json::json!({})),
                    )
                    .await
                {
                    Ok(container_id) => {
                        tracing::info!(
                            "Successfully created and started container: {} (ID: {})",
                            container_name,
                            container_id
                        );
                        ("running".to_string(), Some(container_id))
                    }
                    Err(e) => {
                        tracing::error!("Failed to create container: {}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({
                                "error": format!("Failed to create Docker container: {}", e)
                            })),
                        )
                            .into_response();
                    }
                }
            } else {
                tracing::warn!("No image specified in configuration");
                ("pending".to_string(), None)
            }
        } else {
            tracing::warn!("Docker service not available");
            ("pending".to_string(), None)
        }
    } else {
        ("pending".to_string(), None)
    };

    let new_resource = docker_resources::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        name: ActiveValue::Set(payload.name.clone()),
        resource_type: ActiveValue::Set(payload.resource_type.clone()),
        description: ActiveValue::Set(payload.description.clone()),
        resource_group_id: ActiveValue::Set(payload.resource_group_id),
        configuration: ActiveValue::Set(payload.configuration.clone()),
        status: ActiveValue::Set(initial_status),
        created_by: ActiveValue::Set(Some(user.user_id)),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
        tags: ActiveValue::Set(payload.tags.clone()),
        container_id: ActiveValue::Set(container_id_opt),
        stack_name: ActiveValue::NotSet,
    };

    match new_resource.insert(db).await {
        Ok(resource) => {
            let response = ResourceResponse {
                id: resource.id,
                name: resource.name,
                resource_type: resource.resource_type,
                description: resource.description,
                resource_group_id: resource.resource_group_id,
                resource_group_name: rg.name,
                configuration: resource.configuration,
                status: resource.status,
                created_by: resource.created_by,
                created_at: resource.created_at.to_string(),
                updated_at: resource.updated_at.to_string(),
                tags: resource.tags,
                container_id: resource.container_id,
                stack_name: resource.stack_name,
            };

            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to create resource: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to create resource"
                })),
            )
                .into_response()
        }
    }
}

/// Update an existing resource
async fn update_resource(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateResourceRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    let resource = match DockerResources::find_by_id(id).one(db).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource not found"
                })),
            )
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get resource: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource"
                })),
            )
                .into_response();
        }
    };

    let mut resource_model: docker_resources::ActiveModel = resource.into();

    if let Some(name) = payload.name {
        resource_model.name = ActiveValue::Set(name);
    }
    if let Some(description) = payload.description {
        resource_model.description = ActiveValue::Set(Some(description));
    }
    if let Some(configuration) = payload.configuration {
        resource_model.configuration = ActiveValue::Set(Some(configuration));
    }
    if let Some(status) = payload.status {
        resource_model.status = ActiveValue::Set(status);
    }
    if let Some(tags) = payload.tags {
        resource_model.tags = ActiveValue::Set(Some(tags));
    }

    resource_model.updated_at = ActiveValue::Set(chrono::Utc::now().naive_utc());

    match resource_model.update(db).await {
        Ok(updated) => {
            let rg_name = if let Ok(Some(rg)) =
                ResourceGroups::find_by_id(updated.resource_group_id)
                    .one(db)
                    .await
            {
                rg.name
            } else {
                "Unknown".to_string()
            };

            let response = ResourceResponse {
                id: updated.id,
                name: updated.name,
                resource_type: updated.resource_type,
                description: updated.description,
                resource_group_id: updated.resource_group_id,
                resource_group_name: rg_name,
                configuration: updated.configuration,
                status: updated.status,
                created_by: updated.created_by,
                created_at: updated.created_at.to_string(),
                updated_at: updated.updated_at.to_string(),
                tags: updated.tags,
                container_id: updated.container_id,
                stack_name: updated.stack_name,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update resource: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to update resource"
                })),
            )
                .into_response()
        }
    }
}

/// Delete a resource
async fn delete_resource(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    match DockerResources::find_by_id(id).one(db).await {
        Ok(Some(resource)) => {
            let resource_model: docker_resources::ActiveModel = resource.into();
            match resource_model.delete(db).await {
                Ok(_) => (StatusCode::NO_CONTENT).into_response(),
                Err(e) => {
                    tracing::error!("Failed to delete resource: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "Failed to delete resource"
                        })),
                    )
                        .into_response()
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Resource not found"
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to get resource: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource"
                })),
            )
                .into_response()
        }
    }
}

/// Perform action on a resource (start, stop, restart)
async fn perform_resource_action(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ResourceActionRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the resource
    let resource = match DockerResources::find_by_id(id).one(db).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get resource: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource"
                })),
            )
                .into_response();
        }
    };

    // Validate action
    let new_status = match payload.action.as_str() {
        "start" => "running",
        "stop" => "stopped",
        "restart" => "running",
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Invalid action. Must be 'start', 'stop', or 'restart'"
                })),
            )
                .into_response()
        }
    };

    // Execute Docker command if Docker is available
    let updated_container_id = if let Some(docker) = &state.docker {
        // If container_id doesn't exist and action is "start", create the container first
        let container_id = if resource.container_id.is_none() && payload.action == "start" {
            if let Some(image) = resource
                .configuration
                .as_ref()
                .and_then(|c| c.get("image"))
                .and_then(|i| i.as_str())
            {
                tracing::info!(
                    "No container_id found. Creating new container with image: {}",
                    image
                );

                // Pull image
                match docker.pull_image(image).await {
                    Ok(_) => tracing::info!("Successfully pulled image: {}", image),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to pull image {}: {}. Trying to use local image.",
                            image,
                            e
                        );
                    }
                }

                // Create and start container
                let resource_id_str = resource.id.to_string();
                let container_name = format!(
                    "{}-{}",
                    resource.name.to_lowercase().replace(" ", "-"),
                    &resource_id_str[..8]
                );

                match docker
                    .create_and_start_container(
                        &container_name,
                        image,
                        resource
                            .configuration
                            .as_ref()
                            .unwrap_or(&serde_json::json!({})),
                    )
                    .await
                {
                    Ok(cid) => {
                        tracing::info!(
                            "Successfully created and started container: {} (ID: {})",
                            container_name,
                            cid
                        );
                        Some(cid)
                    }
                    Err(e) => {
                        tracing::error!("Failed to create container: {}", e);
                        return (
                            StatusCode::INTERNAL_SERVER_ERROR,
                            Json(serde_json::json!({
                                "error": format!("Failed to create Docker container: {}", e)
                            })),
                        )
                            .into_response();
                    }
                }
            } else {
                tracing::error!(
                    "No image specified in configuration for resource {}",
                    resource.id
                );
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "No Docker image specified in configuration"
                    })),
                )
                    .into_response();
            }
        } else {
            resource.container_id.clone()
        };

        // Execute the action on the container
        if let Some(cid) = &container_id {
            let docker_result = match payload.action.as_str() {
                "start" => docker.start_container(cid).await,
                "stop" => docker.stop_container(cid).await,
                "restart" => docker.restart_container(cid).await,
                _ => Ok(()),
            };

            if let Err(e) = docker_result {
                tracing::error!("Docker command failed for container {}: {}", cid, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Docker operation failed: {}", e)
                    })),
                )
                    .into_response();
            }

            tracing::info!(
                "Successfully executed '{}' on container: {}",
                payload.action,
                cid
            );
        }

        container_id
    } else {
        tracing::warn!("Docker not available for resource {}", resource.id);
        resource.container_id.clone()
    };

    // Update the resource status and container_id
    let mut resource_active: docker_resources::ActiveModel = resource.into();
    resource_active.status = ActiveValue::Set(new_status.to_string());
    resource_active.updated_at = ActiveValue::Set(chrono::Utc::now().naive_utc());

    // Update container_id if it was created
    if updated_container_id.is_some() {
        resource_active.container_id = ActiveValue::Set(updated_container_id);
    }

    match resource_active.update(db).await {
        Ok(updated) => {
            // Get resource group name
            let rg_name = ResourceGroups::find_by_id(updated.resource_group_id)
                .one(db)
                .await
                .ok()
                .flatten()
                .map(|rg| rg.name)
                .unwrap_or_default();

            let response = ResourceResponse {
                id: updated.id,
                name: updated.name,
                resource_type: updated.resource_type,
                description: updated.description,
                resource_group_id: updated.resource_group_id,
                resource_group_name: rg_name,
                configuration: updated.configuration,
                status: updated.status,
                created_by: updated.created_by,
                created_at: updated.created_at.to_string(),
                updated_at: updated.updated_at.to_string(),
                tags: updated.tags,
                container_id: updated.container_id,
                stack_name: updated.stack_name,
            };

            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to update resource: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to update resource"
                })),
            )
                .into_response()
        }
    }
}

/// Deploy a new Docker container with configuration
async fn deploy_container(
    State(state): State<AppState>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<DeployContainerRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Verify Docker service is available
    let docker = match &state.docker {
        Some(d) => d,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "Docker service not available"
                })),
            )
                .into_response();
        }
    };

    // Verify resource group exists
    let rg = match ResourceGroups::find_by_id(payload.resource_group_id)
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
                .into_response();
        }
        Err(e) => {
            tracing::error!("Failed to get resource group: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource group"
                })),
            )
                .into_response();
        }
    };

    // Build configuration
    let mut configuration = serde_json::json!({
        "image": payload.image,
    });

    if let Some(ports) = payload.ports {
        configuration["ports"] = serde_json::Value::Array(ports);
    }

    if let Some(env) = payload.environment {
        configuration["environment"] = env;
    }

    if let Some(volumes) = payload.volumes {
        configuration["volumes"] = serde_json::Value::Array(volumes);
    }

    tracing::info!(
        "Deploying Docker container '{}' with image '{}'",
        payload.name,
        payload.image
    );

    // Pull the image first
    match docker.pull_image(&payload.image).await {
        Ok(_) => {
            tracing::info!("Successfully pulled image: {}", payload.image);
        }
        Err(e) => {
            tracing::error!("Failed to pull image {}: {}", payload.image, e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Failed to pull image: {}", e)
                })),
            )
                .into_response();
        }
    }

    // Create and start the container
    let container_id = match docker
        .create_and_start_container(&payload.name, &payload.image, &configuration)
        .await
    {
        Ok(id) => {
            tracing::info!("Successfully created and started container: {}", id);
            id
        }
        Err(e) => {
            tracing::error!("Failed to create container: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to create container: {}", e)
                })),
            )
                .into_response();
        }
    };

    // Save to database
    let now = chrono::Utc::now().naive_utc();
    let new_resource = docker_resources::ActiveModel {
        id: ActiveValue::Set(Uuid::new_v4()),
        name: ActiveValue::Set(payload.name.clone()),
        resource_type: ActiveValue::Set("docker-container".to_string()),
        description: ActiveValue::Set(payload.description.clone()),
        resource_group_id: ActiveValue::Set(payload.resource_group_id),
        configuration: ActiveValue::Set(Some(configuration.clone())),
        status: ActiveValue::Set("running".to_string()),
        created_by: ActiveValue::Set(Some(user.user_id)),
        created_at: ActiveValue::Set(now),
        updated_at: ActiveValue::Set(now),
        tags: ActiveValue::NotSet,
        container_id: ActiveValue::Set(Some(container_id.clone())),
        stack_name: ActiveValue::NotSet,
    };

    match new_resource.insert(db).await {
        Ok(resource) => {
            tracing::info!(
                "Deployed container resource: {} ({})",
                resource.name,
                resource.id
            );

            let response = ResourceResponse {
                id: resource.id,
                name: resource.name,
                resource_type: resource.resource_type,
                description: resource.description,
                resource_group_id: resource.resource_group_id,
                resource_group_name: rg.name,
                configuration: resource.configuration,
                status: resource.status,
                created_by: resource.created_by,
                created_at: resource.created_at.to_string(),
                updated_at: resource.updated_at.to_string(),
                tags: resource.tags,
                container_id: resource.container_id,
                stack_name: resource.stack_name,
            };

            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            tracing::error!("Failed to save deployed resource: {}", e);

            // Try to clean up the container
            if let Err(cleanup_err) = docker.stop_container(&container_id).await {
                tracing::error!(
                    "Failed to cleanup container after DB error: {}",
                    cleanup_err
                );
            }

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to save deployed resource"
                })),
            )
                .into_response()
        }
    }
}

/// List all Docker containers from Docker daemon
async fn list_docker_containers(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
) -> impl IntoResponse {
    match &state.docker {
        Some(docker) => match docker.list_containers(true).await {
            Ok(containers) => (StatusCode::OK, Json(containers)).into_response(),
            Err(e) => {
                tracing::error!("Failed to list Docker containers: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to list Docker containers: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Docker service not available"
            })),
        )
            .into_response(),
    }
}

/// Get container logs
async fn get_resource_logs(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the resource
    let resource = match DockerResources::find_by_id(id).one(db).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get resource: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource"
                })),
            )
                .into_response();
        }
    };

    // Check if resource has a container_id
    if resource.container_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Resource does not have a container ID"
            })),
        )
            .into_response();
    }

    let container_id = resource.container_id.unwrap();

    // Get logs from Docker
    match &state.docker {
        Some(docker) => match docker.get_container_logs(&container_id, Some(500)).await {
            Ok(logs) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "logs": logs
                })),
            )
                .into_response(),
            Err(e) => {
                tracing::error!("Failed to get container logs: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to get container logs: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Docker service not available"
            })),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ExecRequest {
    command: String,
}

/// Execute command in container
async fn exec_in_resource(
    State(state): State<AppState>,
    AuthenticatedUser(_user): AuthenticatedUser,
    Path(id): Path<Uuid>,
    Json(payload): Json<ExecRequest>,
) -> impl IntoResponse {
    let db = &state.db_conn;

    // Get the resource
    let resource = match DockerResources::find_by_id(id).one(db).await {
        Ok(Some(r)) => r,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "Resource not found"
                })),
            )
                .into_response()
        }
        Err(e) => {
            tracing::error!("Failed to get resource: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to retrieve resource"
                })),
            )
                .into_response();
        }
    };

    // Check if resource has a container_id
    if resource.container_id.is_none() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Resource does not have a container ID"
            })),
        )
            .into_response();
    }

    let container_id = resource.container_id.unwrap();

    // Parse command - split by whitespace for shell execution
    let cmd_parts: Vec<String> = payload
        .command
        .split_whitespace()
        .map(|s| s.to_string())
        .collect();

    if cmd_parts.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Command cannot be empty"
            })),
        )
            .into_response();
    }

    // Execute command in container
    match &state.docker {
        Some(docker) => match docker.exec_in_container(&container_id, cmd_parts).await {
            Ok(output) => (
                StatusCode::OK,
                Json(serde_json::json!({
                    "output": output
                })),
            )
                .into_response(),
            Err(e) => {
                tracing::error!("Failed to execute command in container: {}", e);
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({
                        "error": format!("Failed to execute command: {}", e)
                    })),
                )
                    .into_response()
            }
        },
        None => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({
                "error": "Docker service not available"
            })),
        )
            .into_response(),
    }
}

pub fn resources_routes() -> Router<AppState> {
    Router::new()
        .route("/resources", get(list_resources))
        .route("/resources/:id", get(get_resource))
        .route("/resources", post(create_resource))
        .route("/resources/deploy", post(deploy_container))
        .route("/resources/:id", put(update_resource))
        .route("/resources/:id", delete(delete_resource))
        .route("/resources/:id/action", post(perform_resource_action))
        .route("/resources/:id/logs", get(get_resource_logs))
        .route("/resources/:id/exec", post(exec_in_resource))
        .route("/docker/containers", get(list_docker_containers))
        .route(
            "/resource-groups/:resource_group_id/resources",
            get(list_resources_by_group),
        )
}
