use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json, Response},
    routing::{get, put},
    Router,
};
use chrono::Utc;
use entity::{
    entities::{
        agent_rg_wg_ports, agents, resource_group_vpn_peers, resource_groups, sites, workloads,
    },
    Agents, ResourceGroupVpnPeers, ResourceGroups, Sites, Workloads,
};
use sea_orm::{
    sea_query::OnConflict, ActiveValue::Set, ColumnTrait, DbConn, DbErr, EntityTrait, QueryFilter,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use shared::ttl_cache::TtlCache;
use shared::vm::VmConfig;
use shared::workload_status::ACTIVE_STATUSES;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use uuid::Uuid;

use super::resource_groups::{db_error, error, ApiError};
use crate::AppState;

#[derive(Debug, Serialize)]
pub struct ResourceGroupPeer {
    pub agent_id: Uuid,
    pub wg_public_key: String,
    pub wg_endpoint: String,
    pub wg_tunnel_ip: String,
    pub wg_listen_port: Option<u16>,
}

pub(super) async fn reported_wg_ports(
    db: &DbConn,
    resource_group_id: Uuid,
) -> Result<HashMap<Uuid, u16>, DbErr> {
    Ok(agent_rg_wg_ports::Entity::find()
        .filter(agent_rg_wg_ports::Column::ResourceGroupId.eq(resource_group_id))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|row| Some((row.agent_id, u16::try_from(row.port).ok()?)))
        .collect())
}

pub(super) fn with_port(endpoint: &str, port: u16) -> String {
    match endpoint.rsplit_once(':') {
        Some((host, _)) => format!("{host}:{port}"),
        None => endpoint.to_string(),
    }
}

#[derive(Debug, Deserialize)]
pub struct ReportWgPortRequest {
    pub port: u16,
}

pub async fn report_resource_group_wg_port(
    agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReportWgPortRequest>,
) -> Result<StatusCode, ApiError> {
    if request.port < 1024 {
        return Err(error(StatusCode::BAD_REQUEST, "port out of range"));
    }
    let row = agent_rg_wg_ports::ActiveModel {
        agent_id: Set(agent.agent_id),
        resource_group_id: Set(id),
        port: Set(i32::from(request.port)),
        updated_at: Set(Utc::now().naive_utc()),
    };
    agent_rg_wg_ports::Entity::insert(row)
        .on_conflict(
            OnConflict::columns([
                agent_rg_wg_ports::Column::AgentId,
                agent_rg_wg_ports::Column::ResourceGroupId,
            ])
            .update_columns([
                agent_rg_wg_ports::Column::Port,
                agent_rg_wg_ports::Column::UpdatedAt,
            ])
            .to_owned(),
        )
        .exec(&state.agent_db)
        .await
        .map_err(db_error("failed to store reported wireguard port"))?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_active_resource_group_ids(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let ids: Vec<Uuid> = ResourceGroups::find()
        .filter(resource_groups::Column::Status.eq("active"))
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to list active resource groups"))?
        .into_iter()
        .map(|rg| rg.id)
        .collect();

    Ok((StatusCode::OK, Json(json!(ids))))
}

pub async fn list_resource_group_peers(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, ApiError> {
    ResourceGroups::find_by_id(id)
        .one(&state.agent_db)
        .await
        .map_err(db_error("failed to find resource group"))?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "resource group not found"))?;

    let hosting_agents = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to list resource group workloads"))?;

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
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to fetch peer agents"))?;

    let ports = reported_wg_ports(&state.agent_db, id)
        .await
        .map_err(db_error("failed to load reported wireguard ports"))?;

    let peers: Vec<ResourceGroupPeer> = agent_rows
        .into_iter()
        .filter_map(|a| {
            Some(ResourceGroupPeer {
                agent_id: a.id,
                wg_public_key: a.wg_public_key?,
                wg_endpoint: a.wg_endpoint?,
                wg_tunnel_ip: a.wg_tunnel_ip?,
                wg_listen_port: ports.get(&a.id).copied(),
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
) -> Result<impl IntoResponse, ApiError> {
    let rows = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::GuestIp.is_not_null())
        .filter(workloads::Column::DesiredState.ne("stopped"))
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to list resource group members"))?;

    let members: Vec<ResourceGroupMember> = rows.into_iter().flat_map(members_of).collect();

    Ok((StatusCode::OK, Json(json!(members))))
}

fn members_of(workload: workloads::Model) -> Vec<ResourceGroupMember> {
    let Some(agent_id) = workload.assigned_agent_id else {
        return Vec::new();
    };
    let extra_ips: Vec<String> = workload
        .vm_config
        .and_then(|config| serde_json::from_value::<VmConfig>(config).ok())
        .map(|config| config.extra_ips().cloned().collect())
        .unwrap_or_default();
    let primary = workload.guest_ip.map(|guest_ip| ResourceGroupMember {
        workload_id: workload.id,
        agent_id,
        service_name: workload.service_name,
        guest_ip,
    });
    primary
        .into_iter()
        .chain(extra_ips.into_iter().map(|guest_ip| ResourceGroupMember {
            workload_id: workload.id,
            agent_id,
            service_name: None,
            guest_ip,
        }))
        .collect()
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
) -> Result<impl IntoResponse, ApiError> {
    let Some(address) = crate::ingress_sync::site_edge_address() else {
        return Ok((StatusCode::OK, Json(json!(Vec::<SiteService>::new()))));
    };
    let services: Vec<SiteService> = Sites::find()
        .filter(sites::Column::ResourceGroupId.eq(id))
        .filter(sites::Column::Mode.eq("static"))
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to list resource group sites"))?
        .into_iter()
        .map(|site| SiteService {
            service_name: site.name,
            address: address.clone(),
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(services))))
}

const MESH_SNAPSHOT_TTL: Duration = Duration::from_secs(5);
const MESH_SNAPSHOT_KEY: &str = "peers";

struct MeshSnapshot {
    etag: String,
    body: String,
}

static MESH_SNAPSHOT: LazyLock<TtlCache<Arc<MeshSnapshot>>> =
    LazyLock::new(|| TtlCache::new(MESH_SNAPSHOT_TTL));

fn etag_of(body: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    body.hash(&mut hasher);
    format!("\"{:x}\"", hasher.finish())
}

async fn load_mesh_snapshot(state: &AppState) -> Result<Arc<MeshSnapshot>, ApiError> {
    if let Some(snapshot) = MESH_SNAPSHOT.get(MESH_SNAPSHOT_KEY) {
        return Ok(snapshot);
    }
    let body = json!(mesh_peers(state).await?).to_string();
    let snapshot = Arc::new(MeshSnapshot {
        etag: etag_of(&body),
        body,
    });
    MESH_SNAPSHOT.insert(MESH_SNAPSHOT_KEY, snapshot.clone());
    Ok(snapshot)
}

pub async fn list_mesh_peers(
    _agent: crate::auth::agent::AgentApiKey,
    State(state): State<AppState>,
    request_headers: HeaderMap,
) -> Result<Response, ApiError> {
    let snapshot = load_mesh_snapshot(&state).await?;
    let etag = [(header::ETAG, snapshot.etag.clone())];
    let known = request_headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok());
    if known == Some(snapshot.etag.as_str()) {
        return Ok((StatusCode::NOT_MODIFIED, etag).into_response());
    }
    let content_type = [(header::CONTENT_TYPE, "application/json".to_string())];
    Ok((StatusCode::OK, etag, content_type, snapshot.body.clone()).into_response())
}

async fn mesh_peers(state: &AppState) -> Result<Vec<ResourceGroupPeer>, ApiError> {
    let agent_rows = Agents::find()
        .filter(agents::Column::WgPublicKey.is_not_null())
        .filter(agents::Column::WgEndpoint.is_not_null())
        .filter(agents::Column::WgTunnelIp.is_not_null())
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to fetch mesh peers"))?;

    Ok(agent_rows
        .into_iter()
        .filter_map(|a| {
            Some(ResourceGroupPeer {
                agent_id: a.id,
                wg_public_key: a.wg_public_key?,
                wg_endpoint: a.wg_endpoint?,
                wg_tunnel_ip: a.wg_tunnel_ip?,
                wg_listen_port: None,
            })
        })
        .collect())
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
) -> Result<impl IntoResponse, ApiError> {
    let peers: Vec<VpnPeerInfo> = ResourceGroupVpnPeers::find()
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(id))
        .all(&state.agent_db)
        .await
        .map_err(db_error("failed to list resource group vpn peers"))?
        .into_iter()
        .map(|p| VpnPeerInfo {
            client_public_key: p.client_public_key,
            client_tunnel_ip: p.client_tunnel_ip,
        })
        .collect();

    Ok((StatusCode::OK, Json(json!(peers))))
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
            "/resource-groups/{id}/wg-port",
            put(report_resource_group_wg_port),
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
