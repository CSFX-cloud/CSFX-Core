use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{delete, get},
    Json, Router,
};
use chrono::Utc;
use entity::{image_registries, image_registry_tokens, ImageRegistries, ImageRegistryTokens};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder, Set,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use site_builder::{images::registry_prefix, validate};
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageResourceGroups, CanViewResourceGroups},
    site_service::SiteError,
    vault_service, AppState,
};

const MAX_REGISTRIES_PER_GROUP: u64 = 10;
const MAX_TOKENS_PER_REGISTRY: u64 = 50;
const MAX_TOKEN_NAME_LENGTH: usize = 64;
const SECRET_PREFIX: &str = "csfxrt_";
const USERNAME_PREFIX: &str = "rt-";

#[derive(Serialize)]
pub struct RegistryView {
    id: Uuid,
    resource_group_id: Uuid,
    name: String,
    prefix: String,
    created_at: chrono::NaiveDateTime,
}

#[derive(Serialize)]
pub struct TokenView {
    id: Uuid,
    name: String,
    username: String,
    scope: String,
    created_at: chrono::NaiveDateTime,
    last_used_at: Option<chrono::NaiveDateTime>,
}

#[derive(Serialize)]
pub struct RegistryDetail {
    #[serde(flatten)]
    registry: RegistryView,
    tokens: Vec<TokenView>,
}

#[derive(Serialize)]
pub struct CreatedToken {
    #[serde(flatten)]
    token: TokenView,
    secret: String,
}

fn invalid(message: &str) -> SiteError {
    SiteError::Invalid(message.to_string())
}

fn view_of(model: image_registries::Model) -> RegistryView {
    RegistryView {
        prefix: registry_prefix(model.resource_group_id, &model.name),
        id: model.id,
        resource_group_id: model.resource_group_id,
        name: model.name,
        created_at: model.created_at,
    }
}

fn token_view(model: image_registry_tokens::Model) -> TokenView {
    TokenView {
        username: format!("{USERNAME_PREFIX}{}", model.id),
        id: model.id,
        name: model.name,
        scope: model.scope,
        created_at: model.created_at,
        last_used_at: model.last_used_at,
    }
}

pub fn hash_secret(secret: &str) -> String {
    Sha256::digest(secret.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub fn new_secret() -> String {
    format!(
        "{SECRET_PREFIX}{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    )
}

async fn load_registry(
    state: &AppState,
    group_id: Uuid,
    registry_id: Uuid,
) -> Result<image_registries::Model, SiteError> {
    vault_service::load_group(state, group_id).await?;
    ImageRegistries::find_by_id(registry_id)
        .filter(image_registries::Column::ResourceGroupId.eq(group_id))
        .one(&state.db_conn)
        .await?
        .ok_or(SiteError::NotFound)
}

async fn builder_post(state: &AppState, path: &str, body: &Value) -> Result<Value, SiteError> {
    let builder = state.site_builder.as_ref().ok_or(SiteError::Unavailable)?;
    Ok(builder.post(path, body).await?)
}

pub async fn list_registries(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
) -> Result<Json<Vec<RegistryView>>, SiteError> {
    vault_service::load_group(&state, group_id).await?;
    let rows = ImageRegistries::find()
        .filter(image_registries::Column::ResourceGroupId.eq(group_id))
        .order_by_asc(image_registries::Column::Name)
        .all(&state.db_conn)
        .await?;
    Ok(Json(rows.into_iter().map(view_of).collect()))
}

#[derive(Deserialize)]
pub struct CreateRegistryRequest {
    name: String,
}

pub async fn create_registry(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<CreateRegistryRequest>,
) -> Result<(StatusCode, Json<RegistryView>), SiteError> {
    validate::site_name(&request.name).map_err(invalid)?;
    vault_service::load_group(&state, group_id).await?;
    let existing = ImageRegistries::find()
        .filter(image_registries::Column::ResourceGroupId.eq(group_id))
        .all(&state.db_conn)
        .await?;
    if existing.len() as u64 >= MAX_REGISTRIES_PER_GROUP {
        return Err(SiteError::Conflict("too many registries in this group"));
    }
    if existing
        .iter()
        .any(|registry| registry.name == request.name)
    {
        return Err(SiteError::Conflict(
            "a registry with this name already exists",
        ));
    }
    let row = image_registries::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        name: Set(request.name),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await?;
    Ok((StatusCode::CREATED, Json(view_of(row))))
}

pub async fn get_registry(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path((group_id, registry_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<RegistryDetail>, SiteError> {
    let registry = load_registry(&state, group_id, registry_id).await?;
    let tokens = ImageRegistryTokens::find()
        .filter(image_registry_tokens::Column::RegistryId.eq(registry.id))
        .order_by_asc(image_registry_tokens::Column::CreatedAt)
        .all(&state.db_conn)
        .await?;
    Ok(Json(RegistryDetail {
        registry: view_of(registry),
        tokens: tokens.into_iter().map(token_view).collect(),
    }))
}

pub async fn delete_registry(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, registry_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, SiteError> {
    let registry = load_registry(&state, group_id, registry_id).await?;
    let prefix = registry_prefix(registry.resource_group_id, &registry.name);
    builder_post(
        &state,
        "/internal/registries/purge",
        &json!({ "prefix": prefix }),
    )
    .await?;
    ImageRegistries::delete_by_id(registry.id)
        .exec(&state.db_conn)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

fn relative_repositories(prefix: &str, listing: &Value) -> Value {
    let repositories: Vec<Value> = listing["repositories"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    let full = item["repository"].as_str().unwrap_or_default();
                    json!({
                        "repository": full.strip_prefix(prefix).unwrap_or(full),
                        "tags": item["tags"],
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    json!({ "repositories": repositories })
}

pub async fn list_images(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path((group_id, registry_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<Value>, SiteError> {
    let registry = load_registry(&state, group_id, registry_id).await?;
    let prefix = registry_prefix(registry.resource_group_id, &registry.name);
    let listing = builder_post(
        &state,
        "/internal/registries/images",
        &json!({ "prefix": prefix }),
    )
    .await?;
    Ok(Json(relative_repositories(&prefix, &listing)))
}

#[derive(Deserialize)]
pub struct ImageQuery {
    repository: String,
    tag: String,
}

pub async fn delete_image(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, registry_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<ImageQuery>,
) -> Result<StatusCode, SiteError> {
    let registry = load_registry(&state, group_id, registry_id).await?;
    let prefix = registry_prefix(registry.resource_group_id, &registry.name);
    let body = json!({ "prefix": prefix, "repository": query.repository, "tag": query.tag });
    builder_post(&state, "/internal/registries/delete-tag", &body).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct CreateTokenRequest {
    name: String,
    scope: String,
}

fn validate_token(request: &CreateTokenRequest) -> Result<(), SiteError> {
    let name = request.name.trim();
    if name.is_empty() || name.len() > MAX_TOKEN_NAME_LENGTH {
        return Err(invalid("token name must be between 1 and 64 characters"));
    }
    if !["pull", "push"].contains(&request.scope.as_str()) {
        return Err(invalid("scope must be pull or push"));
    }
    Ok(())
}

pub async fn create_token(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, registry_id)): Path<(Uuid, Uuid)>,
    Json(request): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<CreatedToken>), SiteError> {
    validate_token(&request)?;
    let registry = load_registry(&state, group_id, registry_id).await?;
    let count = ImageRegistryTokens::find()
        .filter(image_registry_tokens::Column::RegistryId.eq(registry.id))
        .count(&state.db_conn)
        .await?;
    if count >= MAX_TOKENS_PER_REGISTRY {
        return Err(SiteError::Conflict("too many access tokens"));
    }
    let secret = new_secret();
    let row = image_registry_tokens::ActiveModel {
        id: Set(Uuid::new_v4()),
        registry_id: Set(registry.id),
        name: Set(request.name.trim().to_string()),
        token_hash: Set(hash_secret(&secret)),
        scope: Set(request.scope),
        created_at: Set(Utc::now().naive_utc()),
        last_used_at: Set(None),
    }
    .insert(&state.db_conn)
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedToken {
            token: token_view(row),
            secret,
        }),
    ))
}

pub async fn delete_token(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((group_id, registry_id, token_id)): Path<(Uuid, Uuid, Uuid)>,
) -> Result<StatusCode, SiteError> {
    let registry = load_registry(&state, group_id, registry_id).await?;
    let removed = ImageRegistryTokens::delete_many()
        .filter(image_registry_tokens::Column::Id.eq(token_id))
        .filter(image_registry_tokens::Column::RegistryId.eq(registry.id))
        .exec(&state.db_conn)
        .await?;
    if removed.rows_affected == 0 {
        return Err(SiteError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

pub fn registries_routes() -> Router<AppState> {
    let base = "/resource-groups/{group_id}/registries";
    let one = "/resource-groups/{group_id}/registries/{registry_id}";
    Router::new()
        .route(base, get(list_registries).post(create_registry))
        .route(one, get(get_registry).delete(delete_registry))
        .route(
            &format!("{one}/images"),
            get(list_images).delete(delete_image),
        )
        .route(&format!("{one}/tokens"), axum::routing::post(create_token))
        .route(&format!("{one}/tokens/{{token_id}}"), delete(delete_token))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secrets_are_prefixed_unique_and_hash_deterministically() {
        let first = new_secret();
        assert!(first.starts_with(SECRET_PREFIX));
        assert_ne!(first, new_secret());
        assert_eq!(hash_secret("abc"), hash_secret("abc"));
        assert_eq!(hash_secret("abc").len(), 64);
        assert_ne!(hash_secret("abc"), hash_secret("abd"));
    }

    #[test]
    fn listings_are_made_relative_to_the_registry_namespace() {
        let listing = json!({ "repositories": [
            { "repository": "rg-1/main/app/web", "tags": [{ "tag": "v1" }] }
        ]});
        let relative = relative_repositories("rg-1/main/", &listing);
        assert_eq!(relative["repositories"][0]["repository"], "app/web");
        assert_eq!(relative["repositories"][0]["tags"][0]["tag"], "v1");
    }

    #[test]
    fn token_requests_are_validated() {
        let request = |name: &str, scope: &str| CreateTokenRequest {
            name: name.to_string(),
            scope: scope.to_string(),
        };
        assert!(validate_token(&request("ci", "push")).is_ok());
        assert!(validate_token(&request("  ", "push")).is_err());
        assert!(validate_token(&request("ci", "admin")).is_err());
        assert!(validate_token(&request(&"x".repeat(65), "pull")).is_err());
    }
}
