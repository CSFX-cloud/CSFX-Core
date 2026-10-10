use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::Utc;
use entity::{
    identity_providers, idp_group_mappings, role, IdentityProviders, IdpGroupMappings, Role,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::{
    auth::rbac::CanManageIdentityProviders,
    oidc::{client, redirect_uri, secrets},
    AppState,
};

type ApiError = (StatusCode, Json<Value>);

const DEFAULT_SCOPES: &str = "openid profile email";
const MAX_SLUG_LENGTH: usize = 40;

pub fn identity_provider_routes() -> Router<AppState> {
    Router::new()
        .route("/identity-providers", get(list).post(create))
        .route("/identity-providers/{id}", axum::routing::put(update).delete(remove))
        .route("/identity-providers/{id}/test", post(test_connection))
        .route(
            "/identity-providers/{id}/group-mappings",
            get(list_mappings).put(replace_mappings),
        )
}

fn default_scopes() -> String {
    DEFAULT_SCOPES.to_string()
}

fn default_username_claim() -> String {
    "preferred_username".to_string()
}

fn default_email_claim() -> String {
    "email".to_string()
}

fn default_groups_claim() -> String {
    "groups".to_string()
}

fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct ProviderPayload {
    display_name: String,
    issuer_url: String,
    client_id: String,
    client_secret: Option<String>,
    #[serde(default = "default_scopes")]
    scopes: String,
    #[serde(default = "default_username_claim")]
    username_claim: String,
    #[serde(default = "default_email_claim")]
    email_claim: String,
    #[serde(default = "default_groups_claim")]
    groups_claim: String,
    default_role_id: Option<Uuid>,
    #[serde(default = "default_true")]
    auto_provision: bool,
    #[serde(default = "default_true")]
    enabled: bool,
}

#[derive(Deserialize)]
struct CreateProviderRequest {
    slug: String,
    #[serde(flatten)]
    payload: ProviderPayload,
}

#[derive(Serialize)]
struct ProviderResponse {
    id: Uuid,
    slug: String,
    display_name: String,
    issuer_url: String,
    client_id: String,
    scopes: String,
    username_claim: String,
    email_claim: String,
    groups_claim: String,
    default_role_id: Option<Uuid>,
    auto_provision: bool,
    enabled: bool,
    redirect_uri: String,
}

impl From<identity_providers::Model> for ProviderResponse {
    fn from(provider: identity_providers::Model) -> Self {
        Self {
            redirect_uri: redirect_uri(&provider.slug),
            id: provider.id,
            slug: provider.slug,
            display_name: provider.display_name,
            issuer_url: provider.issuer_url,
            client_id: provider.client_id,
            scopes: provider.scopes,
            username_claim: provider.username_claim,
            email_claim: provider.email_claim,
            groups_claim: provider.groups_claim,
            default_role_id: provider.default_role_id,
            auto_provision: provider.auto_provision,
            enabled: provider.enabled,
        }
    }
}

#[derive(Serialize, Deserialize)]
struct MappingPayload {
    external_group: String,
    role_id: Uuid,
    #[serde(default)]
    priority: i32,
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

fn valid_slug(slug: &str) -> bool {
    !slug.is_empty()
        && slug.len() <= MAX_SLUG_LENGTH
        && slug
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

fn secure_issuer(url: &str) -> bool {
    url.starts_with("https://")
        || url.starts_with("http://localhost")
        || url.starts_with("http://127.0.0.1")
}

async fn ensure_role_in_organization(
    db: &DatabaseConnection,
    role_id: Uuid,
    organization_id: Uuid,
) -> Result<(), ApiError> {
    let found = Role::find_by_id(role_id)
        .filter(role::Column::OrganizationId.eq(organization_id))
        .one(db)
        .await
        .map_err(internal("failed to load role"))?;
    found
        .map(|_| ())
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "role not found in organization"))
}

async fn validate_payload(
    db: &DatabaseConnection,
    payload: &ProviderPayload,
    organization_id: Uuid,
) -> Result<(), ApiError> {
    if payload.display_name.trim().is_empty() || payload.client_id.trim().is_empty() {
        return Err(error(StatusCode::BAD_REQUEST, "display_name and client_id are required"));
    }
    if !secure_issuer(&payload.issuer_url) {
        return Err(error(StatusCode::BAD_REQUEST, "issuer_url must use https"));
    }
    match payload.default_role_id {
        Some(role_id) => ensure_role_in_organization(db, role_id, organization_id).await,
        None => Ok(()),
    }
}

fn encrypt_secret(secret: &str) -> Result<String, ApiError> {
    secrets::encrypt(secret).map_err(|cause| {
        tracing::error!(error = %cause, "failed to encrypt client secret");
        error(StatusCode::INTERNAL_SERVER_ERROR, "secret storage failed")
    })
}

async fn load_provider(db: &DatabaseConnection, id: Uuid) -> Result<identity_providers::Model, ApiError> {
    IdentityProviders::find_by_id(id)
        .one(db)
        .await
        .map_err(internal("failed to load identity provider"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "identity provider not found"))
}

async fn list(
    CanManageIdentityProviders(_claims): CanManageIdentityProviders,
    State(state): State<AppState>,
) -> Result<Json<Vec<ProviderResponse>>, ApiError> {
    let providers = IdentityProviders::find()
        .order_by_asc(identity_providers::Column::DisplayName)
        .all(&state.db_conn)
        .await
        .map_err(internal("failed to list identity providers"))?;
    Ok(Json(providers.into_iter().map(Into::into).collect()))
}

async fn create(
    CanManageIdentityProviders(claims): CanManageIdentityProviders,
    State(state): State<AppState>,
    Json(request): Json<CreateProviderRequest>,
) -> Result<(StatusCode, Json<ProviderResponse>), ApiError> {
    let organization_id = organization_id(&state)?;
    let payload = request.payload;
    validate_payload(&state.db_conn, &payload, organization_id).await?;
    if !valid_slug(&request.slug) {
        return Err(error(StatusCode::BAD_REQUEST, "slug must be lowercase alphanumeric with dashes"));
    }
    let secret = payload
        .client_secret
        .as_deref()
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "client_secret is required"))?;
    let created = identity_providers::ActiveModel {
        id: Set(Uuid::new_v4()),
        organization_id: Set(organization_id),
        slug: Set(request.slug),
        display_name: Set(payload.display_name),
        issuer_url: Set(payload.issuer_url),
        client_id: Set(payload.client_id),
        client_secret_encrypted: Set(encrypt_secret(secret)?),
        scopes: Set(payload.scopes),
        username_claim: Set(payload.username_claim),
        email_claim: Set(payload.email_claim),
        groups_claim: Set(payload.groups_claim),
        default_role_id: Set(payload.default_role_id),
        auto_provision: Set(payload.auto_provision),
        enabled: Set(payload.enabled),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&state.db_conn)
    .await
    .map_err(|cause| match cause.to_string().contains("duplicate key") {
        true => error(StatusCode::CONFLICT, "slug already in use"),
        false => internal("failed to create identity provider")(cause),
    })?;
    tracing::info!(provider = %created.slug, actor = %claims.user_id, "identity provider created");
    Ok((StatusCode::CREATED, Json(created.into())))
}

async fn update(
    CanManageIdentityProviders(claims): CanManageIdentityProviders,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ProviderPayload>,
) -> Result<Json<ProviderResponse>, ApiError> {
    let existing = load_provider(&state.db_conn, id).await?;
    validate_payload(&state.db_conn, &payload, existing.organization_id).await?;
    let mut active: identity_providers::ActiveModel = existing.into();
    active.display_name = Set(payload.display_name);
    active.issuer_url = Set(payload.issuer_url);
    active.client_id = Set(payload.client_id);
    active.scopes = Set(payload.scopes);
    active.username_claim = Set(payload.username_claim);
    active.email_claim = Set(payload.email_claim);
    active.groups_claim = Set(payload.groups_claim);
    active.default_role_id = Set(payload.default_role_id);
    active.auto_provision = Set(payload.auto_provision);
    active.enabled = Set(payload.enabled);
    if let Some(secret) = payload.client_secret.as_deref().filter(|s| !s.is_empty()) {
        active.client_secret_encrypted = Set(encrypt_secret(secret)?);
    }
    let updated = active
        .update(&state.db_conn)
        .await
        .map_err(internal("failed to update identity provider"))?;
    tracing::info!(provider = %updated.slug, actor = %claims.user_id, "identity provider updated");
    Ok(Json(updated.into()))
}

async fn remove(
    CanManageIdentityProviders(claims): CanManageIdentityProviders,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, ApiError> {
    let result = IdentityProviders::delete_by_id(id)
        .exec(&state.db_conn)
        .await
        .map_err(internal("failed to delete identity provider"))?;
    if result.rows_affected == 0 {
        return Err(error(StatusCode::NOT_FOUND, "identity provider not found"));
    }
    tracing::info!(provider_id = %id, actor = %claims.user_id, "identity provider deleted");
    Ok(StatusCode::NO_CONTENT)
}

async fn test_connection(
    CanManageIdentityProviders(_claims): CanManageIdentityProviders,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let provider = load_provider(&state.db_conn, id).await?;
    let endpoints = client::discover(&provider.issuer_url).await.map_err(|cause| {
        tracing::warn!(error = %cause, provider = %provider.slug, "identity provider discovery failed");
        error(StatusCode::BAD_GATEWAY, "discovery failed")
    })?;
    Ok(Json(json!({
        "issuer": endpoints.issuer,
        "authorization_endpoint": endpoints.authorization_endpoint,
        "token_endpoint": endpoints.token_endpoint,
        "jwks_uri": endpoints.jwks_uri,
    })))
}

async fn list_mappings(
    CanManageIdentityProviders(_claims): CanManageIdentityProviders,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<Vec<MappingPayload>>, ApiError> {
    let mappings = IdpGroupMappings::find()
        .filter(idp_group_mappings::Column::ProviderId.eq(id))
        .order_by_desc(idp_group_mappings::Column::Priority)
        .all(&state.db_conn)
        .await
        .map_err(internal("failed to list group mappings"))?;
    Ok(Json(
        mappings
            .into_iter()
            .map(|mapping| MappingPayload {
                external_group: mapping.external_group,
                role_id: mapping.role_id,
                priority: mapping.priority,
            })
            .collect(),
    ))
}

async fn replace_mappings(
    CanManageIdentityProviders(claims): CanManageIdentityProviders,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(mappings): Json<Vec<MappingPayload>>,
) -> Result<StatusCode, ApiError> {
    let provider = load_provider(&state.db_conn, id).await?;
    for mapping in &mappings {
        ensure_role_in_organization(&state.db_conn, mapping.role_id, provider.organization_id).await?;
    }
    let rows: Vec<idp_group_mappings::ActiveModel> = mappings
        .into_iter()
        .map(|mapping| idp_group_mappings::ActiveModel {
            id: Set(Uuid::new_v4()),
            provider_id: Set(id),
            external_group: Set(mapping.external_group),
            role_id: Set(mapping.role_id),
            priority: Set(mapping.priority),
        })
        .collect();
    store_mappings(&state.db_conn, id, rows).await?;
    tracing::info!(provider = %provider.slug, actor = %claims.user_id, "group mappings replaced");
    Ok(StatusCode::NO_CONTENT)
}

async fn store_mappings(
    db: &DatabaseConnection,
    provider_id: Uuid,
    rows: Vec<idp_group_mappings::ActiveModel>,
) -> Result<(), ApiError> {
    let failure = internal("failed to store group mappings");
    let txn = db.begin().await.map_err(&failure)?;
    IdpGroupMappings::delete_many()
        .filter(idp_group_mappings::Column::ProviderId.eq(provider_id))
        .exec(&txn)
        .await
        .map_err(&failure)?;
    if !rows.is_empty() {
        IdpGroupMappings::insert_many(rows)
            .exec_without_returning(&txn)
            .await
            .map_err(&failure)?;
    }
    txn.commit().await.map_err(&failure)
}
