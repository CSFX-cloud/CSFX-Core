use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use chrono::Utc;
use entity::{
    entities::{buckets, networks, resource_groups, volumes, workloads},
    Buckets, Networks, ResourceGroups, Volumes, Workloads,
};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DbErr, EntityTrait, PaginatorTrait,
    QueryFilter, QueryOrder,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared::{env_secrets, validate::is_dns_label, workload_status::LIVE_STATUSES};
use uuid::Uuid;

use super::resource_group_network::{
    create_vpn_config, ensure_no_node_address_inside, ensure_tenant_range, parse_cidr,
    revoke_vpn_peer, suggest_cidr, MGMT_TUNNEL_CIDR,
};
use crate::{
    auth::rbac::{CanManageResourceGroups, CanViewResourceGroups},
    AppState,
};

const DEFAULT_ICON: &str = "mdi:cube-outline";
const DEFAULT_COLOR: &str = "#6366f1";
const MAX_ICON_IMAGE_BYTES: usize = 512 * 1024;
const ALLOWED_ICON_IMAGE_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/svg+xml"];

pub(super) type ApiError = (StatusCode, Json<serde_json::Value>);

pub(super) fn error(status: StatusCode, message: impl Into<String>) -> ApiError {
    (status, Json(json!({ "error": message.into() })))
}

pub(super) fn db_error(context: &'static str) -> impl FnOnce(DbErr) -> ApiError {
    move |cause| {
        tracing::error!(error = %cause, "{context}");
        error(StatusCode::INTERNAL_SERVER_ERROR, "database error")
    }
}

pub(super) async fn load_group(
    state: &AppState,
    id: Uuid,
) -> Result<resource_groups::Model, ApiError> {
    ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(state.default_org_id))
        .one(&state.db_conn)
        .await
        .map_err(db_error("failed to find resource group"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "resource group not found"))
}

#[derive(Debug, Deserialize)]
pub struct CreateResourceGroupRequest {
    pub name: String,
    pub description: Option<String>,
    pub internal_cidr: String,
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateResourceGroupRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub pinned: Option<bool>,
    pub preview_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ResourceGroupResponse {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub internal_cidr: String,
    pub status: String,
    pub icon: String,
    pub color: String,
    pub pinned: bool,
    pub has_icon_image: bool,
    pub preview_url: Option<String>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: Option<chrono::NaiveDateTime>,
}

impl From<resource_groups::Model> for ResourceGroupResponse {
    fn from(m: resource_groups::Model) -> Self {
        Self {
            id: m.id,
            organization_id: m.organization_id,
            name: m.name,
            description: m.description,
            internal_cidr: m.internal_cidr,
            status: m.status,
            icon: m.icon,
            color: m.color,
            pinned: m.pinned,
            has_icon_image: m.icon_image.is_some(),
            preview_url: m.preview_url,
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

fn normalize_preview_url(raw: String) -> Result<Option<String>, ApiError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(Some(trimmed.to_string()));
    }
    Err(error(
        StatusCode::BAD_REQUEST,
        "preview_url must start with http:// or https://",
    ))
}

fn validate_name(name: String) -> Result<String, ApiError> {
    let name = name.trim().to_string();
    if is_dns_label(&name, true) {
        return Ok(name);
    }
    Err(error(
        StatusCode::BAD_REQUEST,
        "name must be a dns label of letters, digits and hyphens",
    ))
}

pub async fn list_resource_groups(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let org_id = state.default_org_id;

    let groups = ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .order_by_desc(resource_groups::Column::Pinned)
        .order_by_asc(resource_groups::Column::Name)
        .all(&state.db_conn)
        .await
        .map_err(db_error("failed to list resource groups"))?;

    let resp: Vec<ResourceGroupResponse> = groups.into_iter().map(Into::into).collect();
    Ok((StatusCode::OK, Json(json!(resp))))
}

pub async fn create_resource_group(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Json(req): Json<CreateResourceGroupRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let org_id = state.default_org_id;
    let name = validate_name(req.name)?;

    let requested = parse_cidr(&req.internal_cidr).ok_or_else(|| {
        error(
            StatusCode::BAD_REQUEST,
            format!("invalid CIDR: {}", req.internal_cidr),
        )
    })?;

    ensure_tenant_range(&requested, &req.internal_cidr)?;

    if requested.overlaps(&MGMT_TUNNEL_CIDR) {
        return Err(error(
            StatusCode::CONFLICT,
            format!(
                "CIDR {} overlaps with the management tunnel range (10.100.0.0/16)",
                req.internal_cidr
            ),
        ));
    }

    let existing_groups = ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .all(&state.db_conn)
        .await
        .map_err(db_error("failed to check cidr overlap"))?;

    for group in &existing_groups {
        let Some(other) = parse_cidr(&group.internal_cidr) else {
            continue;
        };

        if requested.overlaps(&other) {
            return Err(error(
                StatusCode::CONFLICT,
                format!(
                    "CIDR {} overlaps with existing resource group {} ({})",
                    req.internal_cidr, group.name, group.internal_cidr
                ),
            ));
        }
    }

    ensure_no_node_address_inside(&state, &requested, &req.internal_cidr).await?;

    let now = Utc::now().naive_utc();
    let model = resource_groups::ActiveModel {
        id: Set(Uuid::new_v4()),
        organization_id: Set(org_id),
        name: Set(name),
        description: Set(req.description),
        internal_cidr: Set(req.internal_cidr),
        status: Set("active".to_string()),
        icon: Set(req.icon.unwrap_or_else(|| DEFAULT_ICON.to_string())),
        color: Set(req.color.unwrap_or_else(|| DEFAULT_COLOR.to_string())),
        pinned: Set(false),
        icon_image: Set(None),
        icon_image_mime: Set(None),
        preview_url: Set(None),
        created_at: Set(now),
        updated_at: Set(None),
        ..Default::default()
    };

    let inserted = model
        .insert(&state.db_conn)
        .await
        .map_err(db_error("failed to create resource group"))?;

    Ok((
        StatusCode::CREATED,
        Json(json!(ResourceGroupResponse::from(inserted))),
    ))
}

pub async fn get_resource_group(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let group = load_group(&state, id).await?;

    Ok((
        StatusCode::OK,
        Json(json!(ResourceGroupResponse::from(group))),
    ))
}

pub async fn update_resource_group(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateResourceGroupRequest>,
) -> Result<impl IntoResponse, ApiError> {
    let group = load_group(&state, id).await?;

    let mut active: resource_groups::ActiveModel = group.into();
    if let Some(name) = req.name {
        active.name = Set(validate_name(name)?);
    }
    if let Some(description) = req.description {
        active.description = Set(Some(description));
    }
    if let Some(icon) = req.icon {
        active.icon = Set(icon);
        active.icon_image = Set(None);
        active.icon_image_mime = Set(None);
    }
    if let Some(color) = req.color {
        active.color = Set(color);
    }
    if let Some(pinned) = req.pinned {
        active.pinned = Set(pinned);
    }
    if let Some(preview_url) = req.preview_url {
        active.preview_url = Set(normalize_preview_url(preview_url)?);
    }
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    let updated = active
        .update(&state.db_conn)
        .await
        .map_err(db_error("failed to update resource group"))?;

    Ok((
        StatusCode::OK,
        Json(json!(ResourceGroupResponse::from(updated))),
    ))
}

pub async fn upload_resource_group_icon_image(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<impl IntoResponse, ApiError> {
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !ALLOWED_ICON_IMAGE_TYPES.contains(&content_type) {
        return Err(error(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "icon image must be png, jpeg or svg",
        ));
    }

    if body.len() > MAX_ICON_IMAGE_BYTES {
        return Err(error(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("icon image must be under {} bytes", MAX_ICON_IMAGE_BYTES),
        ));
    }

    let group = load_group(&state, id).await?;

    let mut active: resource_groups::ActiveModel = group.into();
    active.icon_image = Set(Some(body.to_vec()));
    active.icon_image_mime = Set(Some(content_type.to_string()));
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    let updated = active
        .update(&state.db_conn)
        .await
        .map_err(db_error("failed to store resource group icon image"))?;

    Ok((
        StatusCode::OK,
        Json(json!(ResourceGroupResponse::from(updated))),
    ))
}

pub async fn get_resource_group_icon_image(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let group = load_group(&state, id).await?;

    let (Some(image), Some(mime)) = (group.icon_image, group.icon_image_mime) else {
        return Err(error(
            StatusCode::NOT_FOUND,
            "resource group has no icon image",
        ));
    };

    Ok((StatusCode::OK, [(header::CONTENT_TYPE, mime)], image).into_response())
}

pub async fn delete_resource_group_icon_image(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let group = load_group(&state, id).await?;

    let mut active: resource_groups::ActiveModel = group.into();
    active.icon_image = Set(None);
    active.icon_image_mime = Set(None);
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    let updated = active
        .update(&state.db_conn)
        .await
        .map_err(db_error("failed to clear resource group icon image"))?;

    Ok((
        StatusCode::OK,
        Json(json!(ResourceGroupResponse::from(updated))),
    ))
}

type ContainedResource = (&'static str, Uuid, String);

async fn contained_resources(
    db: &sea_orm::DatabaseConnection,
    group_id: Uuid,
) -> Result<Vec<ContainedResource>, sea_orm::DbErr> {
    let workloads = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(group_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| ("workload", row.id, row.name));
    let volumes = Volumes::find()
        .filter(volumes::Column::ResourceGroupId.eq(group_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| ("volume", row.id, row.name));
    let buckets = Buckets::find()
        .filter(buckets::Column::ResourceGroupId.eq(group_id))
        .all(db)
        .await?
        .into_iter()
        .map(|row| ("bucket", row.id, row.name));
    Ok(workloads.chain(volumes).chain(buckets).collect())
}

fn audit_cascade(actor: Uuid, group_id: Uuid, resources: &[ContainedResource]) {
    for (kind, id, name) in resources {
        shared::audit!(
            actor,
            "resource_groups.cascade_delete",
            *kind,
            id,
            resource_group_id = group_id,
            name = name
        );
    }
}

pub async fn delete_resource_group(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let group = load_group(&state, id).await?;

    let active_workloads = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::Status.is_in(LIVE_STATUSES))
        .count(&state.db_conn)
        .await
        .map_err(db_error("failed to count workloads"))?;

    if active_workloads > 0 {
        return Err(error(
            StatusCode::CONFLICT,
            "resource group has active workloads",
        ));
    }

    let contained = contained_resources(&state.db_conn, id)
        .await
        .map_err(db_error("failed to list contained resources"))?;

    let mut active: resource_groups::ActiveModel = group.into();
    active.status = Set("deleting".to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active
        .update(&state.db_conn)
        .await
        .map_err(db_error("failed to update resource group status"))?;

    if let Err(cause) = crate::vault_service::teardown_resource_group(&state, id).await {
        tracing::error!(resource_group_id = %id, error = %cause, "vault cleanup failed, deletion will be retried");
        return Err(error(
            StatusCode::BAD_GATEWAY,
            "vault cleanup failed, deletion will be retried",
        ));
    }

    ResourceGroups::delete_by_id(id)
        .exec(&state.db_conn)
        .await
        .map_err(db_error("failed to delete resource group"))?;

    audit_cascade(claims.user_id, id, &contained);
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn list_group_resources<E: EntityTrait>(
    state: &AppState,
    id: Uuid,
    column: E::Column,
    context: &'static str,
) -> Result<Vec<E::Model>, ApiError> {
    load_group(state, id).await?;
    E::find()
        .filter(column.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(db_error(context))
}

pub async fn list_resource_group_workloads(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    let mut workloads = list_group_resources::<Workloads>(
        &state,
        id,
        workloads::Column::ResourceGroupId,
        "failed to list workloads",
    )
    .await?;

    for workload in &mut workloads {
        let context = env_secrets::workload_env(workload.id);
        workload.env_vars = workload
            .env_vars
            .as_ref()
            .and_then(|stored| env_secrets::redact(stored, &context));
    }

    Ok(Json(workloads))
}

pub async fn list_resource_group_volumes(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        list_group_resources::<Volumes>(
            &state,
            id,
            volumes::Column::ResourceGroupId,
            "failed to list volumes",
        )
        .await?,
    ))
}

pub async fn list_resource_group_buckets(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        list_group_resources::<Buckets>(
            &state,
            id,
            buckets::Column::ResourceGroupId,
            "failed to list buckets",
        )
        .await?,
    ))
}

pub async fn list_resource_group_networks(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    Ok(Json(
        list_group_resources::<Networks>(
            &state,
            id,
            networks::Column::ResourceGroupId,
            "failed to list networks",
        )
        .await?,
    ))
}

pub fn resource_groups_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/resource-groups",
            get(list_resource_groups).post(create_resource_group),
        )
        .route("/resource-groups/suggest-cidr", get(suggest_cidr))
        .route(
            "/resource-groups/{id}",
            get(get_resource_group)
                .patch(update_resource_group)
                .delete(delete_resource_group),
        )
        .route(
            "/resource-groups/{id}/icon-image",
            get(get_resource_group_icon_image)
                .put(upload_resource_group_icon_image)
                .delete(delete_resource_group_icon_image),
        )
        .route(
            "/resource-groups/{id}/workloads",
            get(list_resource_group_workloads),
        )
        .route(
            "/resource-groups/{id}/volumes",
            get(list_resource_group_volumes),
        )
        .route(
            "/resource-groups/{id}/buckets",
            get(list_resource_group_buckets),
        )
        .route(
            "/resource-groups/{id}/networks",
            get(list_resource_group_networks),
        )
        .route("/resource-groups/{id}/vpn-config", post(create_vpn_config))
        .route(
            "/resource-groups/{id}/vpn-peers/{peer_id}",
            delete(revoke_vpn_peer),
        )
}

#[cfg(test)]
mod name_tests {
    use super::validate_name;

    #[test]
    fn names_must_be_dns_labels() {
        for valid in ["web", "Data-Staging", "a1", " padded "] {
            assert!(validate_name(valid.to_string()).is_ok(), "{valid}");
        }
        for invalid in [
            "",
            "-a",
            "a-",
            "a b",
            "../../Library/x",
            "a/b",
            "a..b",
            "a.b",
            "a\nb",
            &"a".repeat(64),
        ] {
            assert!(validate_name(invalid.to_string()).is_err(), "{invalid:?}");
        }
    }
}
