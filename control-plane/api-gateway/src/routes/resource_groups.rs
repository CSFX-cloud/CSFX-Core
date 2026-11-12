use axum::{
    body::Bytes,
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{delete, get, post},
    Router,
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use entity::{
    entities::{
        agents, buckets, networks, resource_group_vpn_peers, resource_groups, sites, volumes,
        workloads,
    },
    Agents, Buckets, Networks, ResourceGroupVpnPeers, ResourceGroups, Sites, Volumes, Workloads,
};
use ring::rand::{SecureRandom, SystemRandom};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
    QueryOrder,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared::workload_status::ACTIVE_STATUSES;
use std::net::Ipv4Addr;
use uuid::Uuid;

use crate::{
    auth::rbac::{CanManageResourceGroups, CanViewResourceGroups},
    AppState,
};

const DEFAULT_ICON: &str = "mdi:cube-outline";
const DEFAULT_COLOR: &str = "#6366f1";
const MAX_ICON_IMAGE_BYTES: usize = 512 * 1024;
const ALLOWED_ICON_IMAGE_TYPES: [&str; 3] = ["image/png", "image/jpeg", "image/svg+xml"];

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

fn normalize_preview_url(
    raw: String,
) -> Result<Option<String>, (StatusCode, Json<serde_json::Value>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        return Ok(Some(trimmed.to_string()));
    }
    Err((
        StatusCode::BAD_REQUEST,
        Json(json!({ "error": "preview_url must start with http:// or https://" })),
    ))
}

fn get_org_id(state: &AppState) -> Uuid {
    state
        .default_org_id
        .expect("default organization not initialized")
}

pub async fn list_resource_groups(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let groups = ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .order_by_desc(resource_groups::Column::Pinned)
        .order_by_asc(resource_groups::Column::Name)
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource groups");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let resp: Vec<ResourceGroupResponse> = groups.into_iter().map(Into::into).collect();
    Ok((StatusCode::OK, Json(json!(resp))))
}

pub async fn create_resource_group(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Json(req): Json<CreateResourceGroupRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let requested = parse_cidr(&req.internal_cidr).ok_or_else(|| {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "error": format!("invalid CIDR: {}", req.internal_cidr) })),
        )
    })?;

    ensure_tenant_range(&requested, &req.internal_cidr)?;

    if requested.overlaps(&MGMT_TUNNEL_CIDR) {
        return Err((
            StatusCode::CONFLICT,
            Json(
                json!({ "error": format!("CIDR {} overlaps with the management tunnel range (10.100.0.0/16)", req.internal_cidr) }),
            ),
        ));
    }

    let existing_groups = ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to check cidr overlap");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    for group in &existing_groups {
        let Some(other) = parse_cidr(&group.internal_cidr) else {
            continue;
        };

        if requested.overlaps(&other) {
            return Err((
                StatusCode::CONFLICT,
                Json(
                    json!({ "error": format!("CIDR {} overlaps with existing resource group {} ({})", req.internal_cidr, group.name, group.internal_cidr) }),
                ),
            ));
        }
    }

    ensure_no_node_address_inside(&state, &requested, &req.internal_cidr).await?;

    let now = Utc::now().naive_utc();
    let model = resource_groups::ActiveModel {
        id: Set(Uuid::new_v4()),
        organization_id: Set(org_id),
        name: Set(req.name),
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
    };

    let inserted = model.insert(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to create resource group");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!(ResourceGroupResponse::from(inserted))),
    ))
}

const DNS_HOST_OFFSET: u32 = 2;
const VPN_PEER_LOWEST_OFFSET: u32 = 3;
const SUGGESTED_CIDR_BASE: u32 = 0x0A650000;
const SUGGESTED_CIDR_PREFIX: u8 = 24;
const SUGGESTED_CIDR_MAX_SUBNETS: u32 = 256;

const MIN_TENANT_PREFIX: u8 = 16;
const MAX_TENANT_PREFIX: u8 = 29;

const PRIVATE_RANGES: [Cidr; 3] = [
    Cidr {
        network: 0x0A000000,
        prefix_len: 8,
    },
    Cidr {
        network: 0xAC100000,
        prefix_len: 12,
    },
    Cidr {
        network: 0xC0A80000,
        prefix_len: 16,
    },
];

const MGMT_TUNNEL_CIDR: Cidr = Cidr {
    network: 0x0A640000,
    prefix_len: 16,
};

fn bad_cidr(error: String) -> (StatusCode, Json<serde_json::Value>) {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error })))
}

fn ensure_tenant_range(
    requested: &Cidr,
    raw: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    if !PRIVATE_RANGES.iter().any(|range| range.contains(requested)) {
        return Err(bad_cidr(format!(
            "CIDR {raw} must be inside 10.0.0.0/8, 172.16.0.0/12 or 192.168.0.0/16"
        )));
    }
    if !(MIN_TENANT_PREFIX..=MAX_TENANT_PREFIX).contains(&requested.prefix_len) {
        return Err(bad_cidr(format!(
            "CIDR {raw} prefix must be between /{MIN_TENANT_PREFIX} and /{MAX_TENANT_PREFIX}"
        )));
    }
    Ok(())
}

async fn ensure_no_node_address_inside(
    state: &AppState,
    requested: &Cidr,
    raw: &str,
) -> Result<(), (StatusCode, Json<serde_json::Value>)> {
    let agents = Agents::find().all(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to list agents for cidr check");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;
    let collides = agents
        .iter()
        .flat_map(|agent| [&agent.ip_address, &agent.wg_tunnel_ip])
        .flatten()
        .filter_map(|address| address.parse::<Ipv4Addr>().ok())
        .any(|address| requested.contains_address(u32::from(address)));
    if collides {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": format!("CIDR {raw} contains the address of a node") })),
        ));
    }
    Ok(())
}

pub async fn suggest_cidr(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let existing_groups = ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource groups for cidr suggestion");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let existing_cidrs: Vec<Cidr> = existing_groups
        .iter()
        .filter_map(|g| parse_cidr(&g.internal_cidr))
        .collect();

    for subnet_index in 0..SUGGESTED_CIDR_MAX_SUBNETS {
        let network = SUGGESTED_CIDR_BASE + (subnet_index << (32 - SUGGESTED_CIDR_PREFIX));
        let candidate = Cidr {
            network,
            prefix_len: SUGGESTED_CIDR_PREFIX,
        };

        if !existing_cidrs.iter().any(|c| c.overlaps(&candidate)) {
            let [a, b, c, d] = network.to_be_bytes();
            return Ok((
                StatusCode::OK,
                Json(
                    json!({ "internal_cidr": format!("{}.{}.{}.{}/{}", a, b, c, d, SUGGESTED_CIDR_PREFIX) }),
                ),
            ));
        }
    }

    Err((
        StatusCode::CONFLICT,
        Json(json!({ "error": "no free cidr subnet available" })),
    ))
}

pub async fn get_resource_group(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

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
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let mut active: resource_groups::ActiveModel = group.into();
    if let Some(name) = req.name {
        active.name = Set(name);
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

    let updated = active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to update resource group");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

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
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if !ALLOWED_ICON_IMAGE_TYPES.contains(&content_type) {
        return Err((
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            Json(json!({ "error": "icon image must be png, jpeg or svg" })),
        ));
    }

    if body.len() > MAX_ICON_IMAGE_BYTES {
        return Err((
            StatusCode::PAYLOAD_TOO_LARGE,
            Json(
                json!({ "error": format!("icon image must be under {} bytes", MAX_ICON_IMAGE_BYTES) }),
            ),
        ));
    }

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let mut active: resource_groups::ActiveModel = group.into();
    active.icon_image = Set(Some(body.to_vec()));
    active.icon_image_mime = Set(Some(content_type.to_string()));
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    let updated = active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to store resource group icon image");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

    Ok((
        StatusCode::OK,
        Json(json!(ResourceGroupResponse::from(updated))),
    ))
}

pub async fn get_resource_group_icon_image(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let (Some(image), Some(mime)) = (group.icon_image, group.icon_image_mime) else {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "resource group has no icon image" })),
        ));
    };

    Ok((StatusCode::OK, [(header::CONTENT_TYPE, mime)], image).into_response())
}

pub async fn delete_resource_group_icon_image(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let mut active: resource_groups::ActiveModel = group.into();
    active.icon_image = Set(None);
    active.icon_image_mime = Set(None);
    active.updated_at = Set(Some(Utc::now().naive_utc()));

    let updated = active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to clear resource group icon image");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

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
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let active_workloads = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::Status.is_in(["pending", "scheduled", "running"]))
        .count(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to count workloads");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    if active_workloads > 0 {
        return Err((
            StatusCode::CONFLICT,
            Json(json!({ "error": "resource group has active workloads" })),
        ));
    }

    let contained = contained_resources(&state.db_conn, id).await.map_err(|e| {
        tracing::error!(error = %e, "failed to list contained resources");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

    let mut active: resource_groups::ActiveModel = group.into();
    active.status = Set("deleting".to_string());
    active.updated_at = Set(Some(Utc::now().naive_utc()));
    active.update(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to update resource group status");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

    if let Err(error) = crate::vault_service::teardown_resource_group(&state, id).await {
        tracing::error!(resource_group_id = %id, error = %error, "vault cleanup failed, deletion will be retried");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(json!({ "error": "vault cleanup failed, deletion will be retried" })),
        ));
    }

    ResourceGroups::delete_by_id(id)
        .exec(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to delete resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    audit_cascade(claims.user_id, id, &contained);
    Ok(StatusCode::NO_CONTENT.into_response())
}

pub async fn list_resource_group_workloads(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let workloads = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list workloads");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    Ok((StatusCode::OK, Json(json!(workloads))))
}

pub async fn list_resource_group_volumes(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let vols = Volumes::find()
        .filter(volumes::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list volumes");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    Ok((StatusCode::OK, Json(json!(vols))))
}

pub async fn list_resource_group_buckets(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let buckets = Buckets::find()
        .filter(buckets::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list buckets");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    Ok((StatusCode::OK, Json(json!(buckets))))
}

pub async fn list_resource_group_networks(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let nets = Networks::find()
        .filter(networks::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list networks");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    Ok((StatusCode::OK, Json(json!(nets))))
}

#[derive(Debug, Serialize)]
pub struct ResourceGroupPeer {
    pub agent_id: Uuid,
    pub wg_public_key: String,
    pub wg_endpoint: String,
    pub wg_tunnel_ip: String,
}

pub async fn list_active_resource_group_ids(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let ids: Vec<Uuid> = ResourceGroups::find()
        .filter(resource_groups::Column::Status.eq("active"))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list active resource groups");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .into_iter()
        .map(|rg| rg.id)
        .collect();

    Ok((StatusCode::OK, Json(json!(ids))))
}

pub async fn list_resource_group_peers(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    ResourceGroups::find_by_id(id)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let hosting_agents = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource group workloads");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let mut agent_ids: Vec<Uuid> = hosting_agents
        .into_iter()
        .filter_map(|w| w.assigned_agent_id)
        .collect();
    agent_ids.sort_unstable();
    agent_ids.dedup();

    if agent_ids.is_empty() {
        return Ok((StatusCode::OK, Json(json!(Vec::<ResourceGroupPeer>::new()))));
    }

    let agent_rows = Agents::find()
        .filter(agents::Column::Id.is_in(agent_ids))
        .filter(agents::Column::WgPublicKey.is_not_null())
        .filter(agents::Column::WgEndpoint.is_not_null())
        .filter(agents::Column::WgTunnelIp.is_not_null())
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch peer agents");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let peers: Vec<ResourceGroupPeer> = agent_rows
        .into_iter()
        .filter_map(|a| {
            Some(ResourceGroupPeer {
                agent_id: a.id,
                wg_public_key: a.wg_public_key?,
                wg_endpoint: a.wg_endpoint?,
                wg_tunnel_ip: a.wg_tunnel_ip?,
            })
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(peers))))
}

#[derive(Debug, Serialize)]
pub struct ResourceGroupMember {
    pub workload_id: Uuid,
    pub agent_id: Uuid,
    pub service_name: Option<String>,
    pub guest_ip: String,
}

pub async fn list_resource_group_members(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let rows = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::GuestIp.is_not_null())
        .filter(workloads::Column::DesiredState.ne("stopped"))
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource group members");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let members: Vec<ResourceGroupMember> = rows
        .into_iter()
        .filter_map(|w| {
            Some(ResourceGroupMember {
                workload_id: w.id,
                agent_id: w.assigned_agent_id?,
                service_name: w.service_name,
                guest_ip: w.guest_ip?,
            })
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(members))))
}

#[derive(Debug, Serialize)]
pub struct SiteService {
    pub service_name: String,
    pub address: String,
}

pub async fn list_resource_group_site_services(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let Some(address) = crate::ingress_sync::site_edge_address() else {
        return Ok((StatusCode::OK, Json(json!(Vec::<SiteService>::new()))));
    };
    let services: Vec<SiteService> = Sites::find()
        .filter(sites::Column::ResourceGroupId.eq(id))
        .filter(sites::Column::Mode.eq("static"))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource group sites");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .into_iter()
        .map(|site| SiteService {
            service_name: site.name,
            address: address.clone(),
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(services))))
}

pub async fn list_mesh_peers(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let agent_rows = Agents::find()
        .filter(agents::Column::WgPublicKey.is_not_null())
        .filter(agents::Column::WgEndpoint.is_not_null())
        .filter(agents::Column::WgTunnelIp.is_not_null())
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to fetch mesh peers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let peers: Vec<ResourceGroupPeer> = agent_rows
        .into_iter()
        .filter_map(|a| {
            Some(ResourceGroupPeer {
                agent_id: a.id,
                wg_public_key: a.wg_public_key?,
                wg_endpoint: a.wg_endpoint?,
                wg_tunnel_ip: a.wg_tunnel_ip?,
            })
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(peers))))
}

#[derive(Debug, Serialize)]
pub struct VpnPeerInfo {
    pub client_public_key: String,
    pub client_tunnel_ip: String,
}

pub async fn list_resource_group_vpn_peers(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let peers: Vec<VpnPeerInfo> = ResourceGroupVpnPeers::find()
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource group vpn peers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .into_iter()
        .map(|p| VpnPeerInfo {
            client_public_key: p.client_public_key,
            client_tunnel_ip: p.client_tunnel_ip,
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(peers))))
}

pub async fn create_vpn_config(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, (StatusCode, Json<serde_json::Value>)> {
    let org_id = get_org_id(&state);

    let group = ResourceGroups::find_by_id(id)
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to get resource group");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                Json(json!({ "error": "resource group not found" })),
            )
        })?;

    let hosting_agents = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list resource group workloads");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;

    let mut agent_ids: Vec<Uuid> = hosting_agents
        .into_iter()
        .filter_map(|w| w.assigned_agent_id)
        .collect();
    agent_ids.sort_unstable();
    agent_ids.dedup();

    if agent_ids.is_empty() {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            Json(json!({ "error": "no WireGuard-enabled agent online" })),
        ));
    }

    let gateway_agent = Agents::find()
        .filter(agents::Column::Id.is_in(agent_ids))
        .filter(agents::Column::Status.eq("Online"))
        .filter(agents::Column::WgPublicKey.is_not_null())
        .filter(agents::Column::WgEndpoint.is_not_null())
        .order_by_asc(agents::Column::RegisteredAt)
        .one(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to find gateway agent");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "no WireGuard-enabled agent online" })),
            )
        })?;

    let server_pubkey = gateway_agent.wg_public_key.as_deref().unwrap_or("");
    let endpoint = gateway_agent.wg_endpoint.as_deref().unwrap_or("");

    let client_private_key = generate_wg_key().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "failed to generate keypair" })),
        )
    })?;

    let client_public_key = derive_wg_public_key(&client_private_key).ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "failed to derive public key" })),
        )
    })?;

    let existing_ips: Vec<String> = ResourceGroupVpnPeers::find()
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to list existing vpn peers");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?
        .into_iter()
        .map(|p| p.client_tunnel_ip)
        .collect();

    let client_tunnel_ip =
        allocate_vpn_peer_ip(&group.internal_cidr, &existing_ips).ok_or_else(|| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({ "error": "no free address in resource group cidr" })),
            )
        })?;

    let peer = resource_group_vpn_peers::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(id),
        user_id: Set(claims.user_id),
        client_public_key: Set(client_public_key),
        client_tunnel_ip: Set(client_tunnel_ip.clone()),
        created_at: Set(Utc::now().naive_utc()),
    };
    peer.insert(&state.db_conn).await.map_err(|e| {
        tracing::error!(error = %e, "failed to store vpn peer");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "database error" })),
        )
    })?;

    let dns = dns_host_ip(&group.internal_cidr).unwrap_or_else(|| "1.1.1.1".to_string());
    let search_domain = format!("svc.{}.internal", id);

    let config = format!(
        "[Interface]\nPrivateKey = {client_private_key}\nAddress = {client_tunnel_ip}/32\nDNS = {dns}, {search_domain}\n\n[Peer]\nPublicKey = {server_pubkey}\nEndpoint = {endpoint}\nAllowedIPs = {cidr}\nPersistentKeepalive = 25\n",
        client_private_key = client_private_key,
        client_tunnel_ip = client_tunnel_ip,
        dns = dns,
        server_pubkey = server_pubkey,
        endpoint = endpoint,
        cidr = group.internal_cidr,
    );

    let filename = format!("csfx-{}.conf", group.name.to_lowercase().replace(' ', "-"));

    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
            (
                header::CONTENT_DISPOSITION,
                &format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        config,
    )
        .into_response())
}

pub async fn revoke_vpn_peer(
    CanManageResourceGroups(_claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path((id, peer_id)): Path<(Uuid, Uuid)>,
) -> Result<StatusCode, (StatusCode, Json<serde_json::Value>)> {
    let result = ResourceGroupVpnPeers::delete_many()
        .filter(resource_group_vpn_peers::Column::Id.eq(peer_id))
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(id))
        .exec(&state.db_conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to revoke vpn peer");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": "database error" })),
            )
        })?;
    if result.rows_affected == 0 {
        return Err((
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "vpn peer not found" })),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

fn generate_wg_key() -> Result<String, ring::error::Unspecified> {
    let rng = SystemRandom::new();
    let mut key_bytes = [0u8; 32];
    rng.fill(&mut key_bytes)?;
    key_bytes[0] &= 248;
    key_bytes[31] &= 127;
    key_bytes[31] |= 64;
    Ok(B64.encode(key_bytes))
}

fn derive_wg_public_key(private_key_b64: &str) -> Option<String> {
    let bytes = B64.decode(private_key_b64).ok()?;
    let bytes: [u8; 32] = bytes.try_into().ok()?;
    let secret = x25519_dalek::StaticSecret::from(bytes);
    let public = x25519_dalek::PublicKey::from(&secret);
    Some(B64.encode(public.as_bytes()))
}

fn allocate_vpn_peer_ip(cidr: &str, taken: &[String]) -> Option<String> {
    let parsed = parse_cidr(cidr)?;
    let host_count = 1u32 << (32 - parsed.prefix_len);
    (VPN_PEER_LOWEST_OFFSET..host_count.saturating_sub(1))
        .rev()
        .map(|offset| Ipv4Addr::from(parsed.network + offset).to_string())
        .find(|candidate| !taken.contains(candidate))
}

struct Cidr {
    network: u32,
    prefix_len: u8,
}

impl Cidr {
    fn mask(&self) -> u32 {
        if self.prefix_len == 0 {
            0
        } else {
            u32::MAX << (32 - self.prefix_len)
        }
    }

    fn contains(&self, other: &Cidr) -> bool {
        other.prefix_len >= self.prefix_len && self.contains_address(other.network)
    }

    fn contains_address(&self, address: u32) -> bool {
        (address & self.mask()) == self.network
    }

    fn overlaps(&self, other: &Cidr) -> bool {
        let shared_prefix = self.prefix_len.min(other.prefix_len);
        let shared_mask = if shared_prefix == 0 {
            0
        } else {
            u32::MAX << (32 - shared_prefix)
        };
        (self.network & shared_mask) == (other.network & shared_mask)
    }
}

fn parse_cidr(cidr: &str) -> Option<Cidr> {
    let (addr, prefix) = cidr.split_once('/')?;
    let octets: Vec<u8> = addr.split('.').filter_map(|o| o.parse().ok()).collect();
    if octets.len() != 4 {
        return None;
    }
    let prefix_len: u8 = prefix.parse().ok()?;
    if prefix_len > 32 {
        return None;
    }
    let network = u32::from_be_bytes([octets[0], octets[1], octets[2], octets[3]]);
    let cidr = Cidr {
        network,
        prefix_len,
    };
    let masked = network & cidr.mask();
    if masked != network {
        return None;
    }
    Some(cidr)
}

fn dns_host_ip(cidr: &str) -> Option<String> {
    let parsed = parse_cidr(cidr)?;
    Some(Ipv4Addr::from(parsed.network + DNS_HOST_OFFSET).to_string())
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

pub fn resource_groups_agent_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/resource-groups/agent/active-ids",
            get(list_active_resource_group_ids),
        )
        .route("/agents/mesh-peers", get(list_mesh_peers))
        .route(
            "/resource-groups/{id}/peers",
            get(list_resource_group_peers),
        )
        .route(
            "/resource-groups/{id}/members",
            get(list_resource_group_members),
        )
        .route(
            "/resource-groups/{id}/site-services",
            get(list_resource_group_site_services),
        )
        .route(
            "/resource-groups/{id}/vpn-peers",
            get(list_resource_group_vpn_peers),
        )
}

#[cfg(test)]
mod vpn_tests {
    use super::{allocate_vpn_peer_ip, dns_host_ip};

    #[test]
    fn dns_is_the_second_host_of_the_group() {
        assert_eq!(dns_host_ip("10.20.0.0/24").as_deref(), Some("10.20.0.2"));
    }

    #[test]
    fn peers_are_allocated_from_the_top_and_skip_taken_addresses() {
        let taken = vec!["10.20.0.254".to_string()];
        assert_eq!(
            allocate_vpn_peer_ip("10.20.0.0/24", &[]).as_deref(),
            Some("10.20.0.254")
        );
        assert_eq!(
            allocate_vpn_peer_ip("10.20.0.0/24", &taken).as_deref(),
            Some("10.20.0.253")
        );
    }

    #[test]
    fn peers_never_take_the_infrastructure_addresses() {
        let taken: Vec<String> = (3..=254).map(|host| format!("10.20.0.{host}")).collect();
        assert_eq!(allocate_vpn_peer_ip("10.20.0.0/24", &taken), None);
    }
}
