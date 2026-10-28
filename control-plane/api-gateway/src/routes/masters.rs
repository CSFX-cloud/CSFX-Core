use axum::{
    extract::State,
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use chrono::{Duration, Utc};
use entity::entities::agents;
use etcd_client::Client;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use uuid::Uuid;

use super::{
    cluster::{error, internal, ApiError},
    cluster_vip::effective_vip,
};
use crate::{
    auth::{agent::AgentApiKey, rbac::CanManageSystem},
    cluster_bundle::{BundleError, ClusterSecrets, ClusterVip},
    etcd_members::{self, EtcdMember},
    tls, AppState,
};

pub const ROLE_MASTER: &str = "master";
const JOIN_WINDOW_MINUTES: i64 = 30;
const PATRONI_MEMBER_KEY_PREFIX: &str = "/service/csfx/members/";
const DEFAULT_ETCD_ENDPOINT: &str = "http://localhost:2379";

#[derive(Serialize)]
pub struct JoinBundle {
    ca_cert_pem: String,
    ca_key_pem: String,
    #[serde(flatten)]
    secrets: ClusterSecrets,
    etcd_member_id: u64,
    etcd_name: String,
    etcd_initial_cluster: String,
    vip: Option<ClusterVip>,
}

pub fn member_name(agent_id: Uuid) -> String {
    format!("csfx-{}", &agent_id.simple().to_string()[..8])
}

async fn etcd_client() -> Result<Client, ApiError> {
    let endpoints =
        std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| DEFAULT_ETCD_ENDPOINT.into());
    Client::connect(endpoints.split(',').collect::<Vec<_>>(), None)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to connect to etcd");
            error(StatusCode::SERVICE_UNAVAILABLE, "etcd unavailable")
        })
}

pub fn peer_url(tunnel_ip: &str) -> String {
    format!("http://{tunnel_ip}:2380")
}

async fn load_agent(state: &AppState, agent_id: Uuid) -> Result<agents::Model, ApiError> {
    agents::Entity::find_by_id(agent_id)
        .one(&state.db_conn)
        .await
        .map_err(internal)?
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "node not found"))
}

pub(super) async fn registered_master(
    state: &AppState,
    agent_id: Uuid,
) -> Result<agents::Model, ApiError> {
    let node = load_agent(state, agent_id).await?;
    if node.role != ROLE_MASTER {
        return Err(error(
            StatusCode::FORBIDDEN,
            "node was not registered as master",
        ));
    }
    Ok(node)
}

async fn joining_master(state: &AppState, agent_id: Uuid) -> Result<agents::Model, ApiError> {
    let node = registered_master(state, agent_id).await?;
    let age = Utc::now().naive_utc() - node.registered_at;
    if age > Duration::minutes(JOIN_WINDOW_MINUTES) {
        return Err(error(StatusCode::FORBIDDEN, "master join window expired"));
    }
    Ok(node)
}

fn tunnel_ip(node: &agents::Model) -> Result<&str, ApiError> {
    node.wg_tunnel_ip
        .as_deref()
        .ok_or_else(|| error(StatusCode::CONFLICT, "node has no management tunnel ip"))
}

async fn master_peer_urls(state: &AppState) -> Result<HashSet<String>, ApiError> {
    let masters = agents::Entity::find()
        .filter(agents::Column::Role.eq(ROLE_MASTER))
        .all(&state.db_conn)
        .await
        .map_err(internal)?;
    Ok(masters
        .iter()
        .filter_map(|master| master.wg_tunnel_ip.as_deref())
        .map(peer_url)
        .collect())
}

fn find_member<'a>(members: &'a [EtcdMember], url: &str) -> Option<&'a EtcdMember> {
    members
        .iter()
        .find(|member| member.peer_urls.iter().any(|candidate| candidate == url))
}

pub async fn join_master(
    agent: AgentApiKey,
    State(state): State<AppState>,
) -> Result<Json<JoinBundle>, ApiError> {
    let node = joining_master(&state, agent.agent_id).await?;
    let url = peer_url(tunnel_ip(&node)?);
    let secrets = ClusterSecrets::load().map_err(internal)?;
    let (ca_cert_pem, ca_key_pem) = tls::load_ca_pem()
        .await
        .ok_or_else(|| internal(BundleError::MissingCa))?;

    let mut etcd = etcd_client().await?;
    let members = etcd_members::list_members(&mut etcd)
        .await
        .map_err(internal)?;
    if find_member(&members, &url).is_some_and(|member| !member.is_learner) {
        return Err(error(
            StatusCode::CONFLICT,
            "node already joined the etcd cluster",
        ));
    }

    if etcd_members::has_loopback_voter(&members) {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "existing master still advertises a loopback peer url",
        ));
    }
    let known_urls = master_peer_urls(&state).await?;
    let stale = etcd_members::stale_learner_ids(&members, &known_urls);
    for id in &stale {
        etcd_members::remove(&mut etcd, *id)
            .await
            .map_err(internal)?;
    }
    let active: Vec<_> = members
        .iter()
        .filter(|member| !stale.contains(&member.id))
        .cloned()
        .collect();
    if etcd_members::other_learner_pending(&active, &url) {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "another master join is still in progress",
        ));
    }

    let name = member_name(node.id);
    let plan = etcd_members::add_learner(&mut etcd, &name, &url)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, agent_id = %node.id, "etcd refused the learner");
            error(StatusCode::CONFLICT, "etcd refused the new learner")
        })?;

    tracing::info!(agent_id = %node.id, member_id = plan.member_id, "master join bundle issued");

    Ok(Json(JoinBundle {
        ca_cert_pem,
        ca_key_pem,
        secrets,
        etcd_member_id: plan.member_id,
        etcd_name: name,
        etcd_initial_cluster: plan.initial_cluster,
        vip: effective_vip(&state).await?,
    }))
}

pub async fn promote_master(
    agent: AgentApiKey,
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    let node = registered_master(&state, agent.agent_id).await?;
    let url = peer_url(tunnel_ip(&node)?);

    let mut etcd = etcd_client().await?;
    let members = etcd_members::list_members(&mut etcd)
        .await
        .map_err(internal)?;
    let member = find_member(&members, &url)
        .ok_or_else(|| error(StatusCode::NOT_FOUND, "etcd member not found"))?;
    if !member.is_learner {
        return Ok(StatusCode::NO_CONTENT);
    }

    etcd_members::promote(&mut etcd, member.id)
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, member_id = member.id, "etcd learner promotion refused");
            error(StatusCode::CONFLICT, "etcd learner is not in sync yet")
        })?;

    tracing::info!(agent_id = %node.id, member_id = member.id, "master promoted to etcd voter");
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_masters(
    CanManageSystem(_claims): CanManageSystem,
) -> Result<Json<Value>, ApiError> {
    let mut etcd = etcd_client().await?;
    let members = etcd_members::list_members(&mut etcd)
        .await
        .map_err(internal)?;
    let voters = etcd_members::voter_count(&members);
    let members: Vec<Value> = members
        .iter()
        .map(|member| {
            json!({
                "id": member.id.to_string(),
                "name": member.name,
                "peer_urls": member.peer_urls,
                "is_learner": member.is_learner,
            })
        })
        .collect();

    Ok(Json(json!({ "voters": voters, "members": members })))
}

pub async fn plan_removal(node: &agents::Model) -> Result<Option<u64>, ApiError> {
    if node.role != ROLE_MASTER {
        return Ok(None);
    }
    let Some(ip) = node.wg_tunnel_ip.as_deref() else {
        return Ok(None);
    };

    let mut etcd = etcd_client().await?;
    let members = etcd_members::list_members(&mut etcd)
        .await
        .map_err(internal)?;
    let Some(member) = find_member(&members, &peer_url(ip)) else {
        return Ok(None);
    };
    etcd_members::check_removal(&members, member.id)
        .map_err(|e| error(StatusCode::CONFLICT, &e.to_string()))?;
    Ok(Some(member.id))
}

pub async fn finish_removal(member_id: u64, node: &agents::Model) -> Result<(), ApiError> {
    let mut etcd = etcd_client().await?;
    etcd_members::remove(&mut etcd, member_id)
        .await
        .map_err(internal)?;
    for name in [node.hostname.clone(), member_name(node.id)] {
        etcd.delete(format!("{PATRONI_MEMBER_KEY_PREFIX}{name}"), None)
            .await
            .map_err(internal)?;
    }
    Ok(())
}

pub fn masters_routes() -> Router<AppState> {
    Router::new()
        .route("/cluster/masters", get(list_masters))
        .route("/cluster/masters/join", post(join_master))
        .route("/cluster/masters/promote", post(promote_master))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn member_name_is_stable_and_distinct_per_agent() {
        let first = Uuid::parse_str("0a1b2c3d-1111-4222-8333-444455556666").unwrap();
        let second = Uuid::parse_str("9f8e7d6c-1111-4222-8333-444455556666").unwrap();
        assert_eq!(member_name(first), "csfx-0a1b2c3d");
        assert_ne!(member_name(first), member_name(second));
    }
}
