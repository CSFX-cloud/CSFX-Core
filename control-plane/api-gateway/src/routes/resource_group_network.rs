use axum::{
    extract::{Path, State},
    http::{header, StatusCode},
    response::{IntoResponse, Json, Response},
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use entity::{
    entities::{agents, resource_group_vpn_peers, resource_groups, workloads},
    Agents, ResourceGroupVpnPeers, ResourceGroups, Workloads,
};
use ring::rand::{SecureRandom, SystemRandom};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter, QueryOrder,
};
use serde_json::json;
use shared::workload_status::ACTIVE_STATUSES;
use std::net::Ipv4Addr;
use uuid::Uuid;

use super::resource_group_agent::{reported_wg_ports, with_port};
use super::resource_groups::{db_error, error, load_group, ApiError};
use crate::{
    auth::rbac::{CanManageResourceGroups, CanViewResourceGroups},
    AppState,
};

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

pub(super) const MGMT_TUNNEL_CIDR: Cidr = Cidr {
    network: 0x0A640000,
    prefix_len: 16,
};

fn bad_cidr(error: String) -> ApiError {
    (StatusCode::BAD_REQUEST, Json(json!({ "error": error })))
}

pub(super) fn ensure_tenant_range(requested: &Cidr, raw: &str) -> Result<(), ApiError> {
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

pub(super) async fn ensure_no_node_address_inside(
    state: &AppState,
    requested: &Cidr,
    raw: &str,
) -> Result<(), ApiError> {
    let agents = Agents::find()
        .all(&state.db_conn)
        .await
        .map_err(db_error("failed to list agents for cidr check"))?;
    let collides = agents
        .iter()
        .flat_map(|agent| [&agent.ip_address, &agent.wg_tunnel_ip])
        .flatten()
        .filter_map(|address| address.parse::<Ipv4Addr>().ok())
        .any(|address| requested.contains_address(u32::from(address)));
    if collides {
        return Err(error(
            StatusCode::CONFLICT,
            format!("CIDR {raw} contains the address of a node"),
        ));
    }
    Ok(())
}

pub async fn suggest_cidr(
    CanViewResourceGroups(_claims): CanViewResourceGroups,
    State(state): State<AppState>,
) -> Result<impl IntoResponse, ApiError> {
    let org_id = state.default_org_id;

    let existing_groups = ResourceGroups::find()
        .filter(resource_groups::Column::OrganizationId.eq(org_id))
        .all(&state.db_conn)
        .await
        .map_err(db_error(
            "failed to list resource groups for cidr suggestion",
        ))?;

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

    Err(error(StatusCode::CONFLICT, "no free cidr subnet available"))
}

pub async fn create_vpn_config(
    CanManageResourceGroups(claims): CanManageResourceGroups,
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Response, ApiError> {
    let group = load_group(&state, id).await?;

    let hosting_agents = Workloads::find()
        .filter(workloads::Column::ResourceGroupId.eq(id))
        .filter(workloads::Column::AssignedAgentId.is_not_null())
        .filter(workloads::Column::Status.is_in(ACTIVE_STATUSES))
        .all(&state.db_conn)
        .await
        .map_err(db_error("failed to list resource group workloads"))?;

    let mut agent_ids: Vec<Uuid> = hosting_agents
        .into_iter()
        .filter_map(|w| w.assigned_agent_id)
        .collect();
    agent_ids.sort_unstable();
    agent_ids.dedup();

    if agent_ids.is_empty() {
        return Err(error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no WireGuard-enabled agent online",
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
        .map_err(db_error("failed to find gateway agent"))?
        .ok_or_else(|| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "no WireGuard-enabled agent online",
            )
        })?;

    let server_pubkey = gateway_agent.wg_public_key.as_deref().unwrap_or("");
    let reported_port = reported_wg_ports(&state.db_conn, id)
        .await
        .map_err(db_error("failed to load reported wireguard ports"))?
        .get(&gateway_agent.id)
        .copied();
    let advertised = gateway_agent.wg_endpoint.as_deref().unwrap_or("");
    let endpoint = match reported_port {
        Some(port) => with_port(advertised, port),
        None => advertised.to_string(),
    };

    let client_private_key = generate_wg_key().map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to generate keypair",
        )
    })?;

    let client_public_key = derive_wg_public_key(&client_private_key).ok_or_else(|| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "failed to derive public key",
        )
    })?;

    let existing_ips: Vec<String> = ResourceGroupVpnPeers::find()
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(id))
        .all(&state.db_conn)
        .await
        .map_err(db_error("failed to list existing vpn peers"))?
        .into_iter()
        .map(|p| p.client_tunnel_ip)
        .collect();

    let client_tunnel_ip =
        allocate_vpn_peer_ip(&group.internal_cidr, &existing_ips).ok_or_else(|| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "no free address in resource group cidr",
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
    peer.insert(&state.db_conn)
        .await
        .map_err(db_error("failed to store vpn peer"))?;

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
) -> Result<StatusCode, ApiError> {
    let result = ResourceGroupVpnPeers::delete_many()
        .filter(resource_group_vpn_peers::Column::Id.eq(peer_id))
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(id))
        .exec(&state.db_conn)
        .await
        .map_err(db_error("failed to revoke vpn peer"))?;
    if result.rows_affected == 0 {
        return Err(error(StatusCode::NOT_FOUND, "vpn peer not found"));
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

pub(super) struct Cidr {
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

    pub(super) fn overlaps(&self, other: &Cidr) -> bool {
        let shared_prefix = self.prefix_len.min(other.prefix_len);
        let shared_mask = if shared_prefix == 0 {
            0
        } else {
            u32::MAX << (32 - shared_prefix)
        };
        (self.network & shared_mask) == (other.network & shared_mask)
    }
}

pub(super) fn parse_cidr(cidr: &str) -> Option<Cidr> {
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
