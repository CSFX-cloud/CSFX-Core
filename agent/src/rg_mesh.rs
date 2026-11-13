use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tracing::warn;

use crate::client::{ApiClient, ResourceGroupMember, ResourceGroupPeer, SiteService};
use crate::local_state::LocalState;
use crate::rg_dns::RgDnsRegistry;
use crate::rg_network;
use crate::spec::rg_wireguard_port;
use crate::wireguard::{self, Peer};

pub async fn sync(
    client: &ApiClient,
    api_key: &str,
    agent_id: uuid::Uuid,
    resource_group_ids: &[String],
    dns_registry: &RgDnsRegistry,
    local_state: &LocalState,
) {
    for resource_group_id in resource_group_ids {
        report_wg_port(client, api_key, resource_group_id, local_state).await;
        if let Err(e) = sync_group(
            client,
            api_key,
            &agent_id.to_string(),
            resource_group_id,
            dns_registry,
        )
        .await
        {
            warn!(resource_group_id = %resource_group_id, error = %e, "Failed to sync resource group mesh");
        }
    }
}

async fn report_wg_port(
    client: &ApiClient,
    api_key: &str,
    resource_group_id: &str,
    local_state: &LocalState,
) {
    let Some(port) = local_state.assigned_wg_port(resource_group_id).await else {
        return;
    };
    if let Err(e) = client
        .report_resource_group_wg_port(api_key, resource_group_id, port)
        .await
    {
        warn!(resource_group_id = %resource_group_id, port, error = %e, "Failed to report wireguard port");
    }
}

async fn sync_group(
    client: &ApiClient,
    api_key: &str,
    agent_id: &str,
    resource_group_id: &str,
    dns_registry: &RgDnsRegistry,
) -> Result<()> {
    let peers = client
        .fetch_resource_group_peers(api_key, resource_group_id)
        .await?;
    let members = client
        .fetch_resource_group_members(api_key, resource_group_id)
        .await?;

    let sites = fetch_site_services(client, api_key, resource_group_id).await;

    let iface = wireguard::rg_interface_name(resource_group_id);
    let wg_peers = build_peers(resource_group_id, agent_id, peers, &members);
    wireguard::reconcile_peers(&iface, &wg_peers).await?;
    let added = wireguard::sync_host_routes(&iface, &remote_guest_ips(agent_id, &members)).await?;
    announce_moved_guests(resource_group_id, &added).await;

    dns_registry
        .sync_services(resource_group_id, service_records(&members, &sites))
        .await
}

async fn fetch_site_services(
    client: &ApiClient,
    api_key: &str,
    resource_group_id: &str,
) -> Vec<SiteService> {
    match client
        .fetch_resource_group_site_services(api_key, resource_group_id)
        .await
    {
        Ok(sites) => sites,
        Err(e) => {
            warn!(resource_group_id = %resource_group_id, error = %e, "Failed to fetch site services");
            Vec::new()
        }
    }
}

async fn announce_moved_guests(resource_group_id: &str, guest_ips: &[String]) {
    for guest_ip in guest_ips {
        if let Err(e) = rg_network::announce_remote_guest(resource_group_id, guest_ip).await {
            warn!(resource_group_id = %resource_group_id, guest_ip = %guest_ip, error = %e, "Failed to announce remote guest");
        }
    }
}

fn build_peers(
    resource_group_id: &str,
    agent_id: &str,
    peers: Vec<ResourceGroupPeer>,
    members: &[ResourceGroupMember],
) -> Vec<Peer> {
    let fallback_port = rg_wireguard_port(resource_group_id);
    peers
        .into_iter()
        .filter(|peer| peer.agent_id != agent_id)
        .filter_map(|peer| {
            let endpoint = with_port(
                &peer.wg_endpoint,
                peer.wg_listen_port.unwrap_or(fallback_port),
            )?;
            let mut allowed_ips = vec![format!("{}/32", peer.wg_tunnel_ip)];
            allowed_ips.extend(
                members
                    .iter()
                    .filter(|member| member.agent_id == peer.agent_id)
                    .map(|member| format!("{}/32", member.guest_ip)),
            );
            Some(Peer {
                public_key: peer.wg_public_key,
                endpoint: Some(endpoint),
                allowed_ips: allowed_ips.join(","),
            })
        })
        .collect()
}

fn with_port(endpoint: &str, port: u16) -> Option<String> {
    let (host, _) = endpoint.rsplit_once(':')?;
    Some(format!("{}:{}", host, port))
}

fn remote_guest_ips(agent_id: &str, members: &[ResourceGroupMember]) -> HashSet<String> {
    members
        .iter()
        .filter(|member| member.agent_id != agent_id)
        .map(|member| member.guest_ip.clone())
        .collect()
}

fn service_records(
    members: &[ResourceGroupMember],
    sites: &[SiteService],
) -> HashMap<String, String> {
    sites
        .iter()
        .map(|site| (site.service_name.clone(), site.address.clone()))
        .chain(
            members
                .iter()
                .filter_map(|member| Some((member.service_name.clone()?, member.guest_ip.clone()))),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(agent_id: &str, service_name: Option<&str>, guest_ip: &str) -> ResourceGroupMember {
        ResourceGroupMember {
            agent_id: agent_id.to_string(),
            service_name: service_name.map(str::to_string),
            guest_ip: guest_ip.to_string(),
        }
    }

    fn peer(agent_id: &str, tunnel_ip: &str) -> ResourceGroupPeer {
        ResourceGroupPeer {
            agent_id: agent_id.to_string(),
            wg_public_key: format!("key-{}", agent_id),
            wg_endpoint: "192.168.1.5:51820".to_string(),
            wg_tunnel_ip: tunnel_ip.to_string(),
            wg_listen_port: None,
        }
    }

    #[test]
    fn peers_carry_the_guest_addresses_of_their_node_on_the_group_port() {
        let members = vec![
            member("a", Some("web"), "10.0.0.3"),
            member("b", Some("api"), "10.0.0.4"),
            member("b", None, "10.0.0.5"),
        ];
        let peers = build_peers(
            "group",
            "a",
            vec![peer("a", "10.100.0.2"), peer("b", "10.100.0.3")],
            &members,
        );

        assert_eq!(peers.len(), 1);
        assert_eq!(
            peers[0].allowed_ips,
            "10.100.0.3/32,10.0.0.4/32,10.0.0.5/32"
        );
        assert_eq!(
            peers[0].endpoint,
            Some(format!("192.168.1.5:{}", rg_wireguard_port("group")))
        );
    }

    #[test]
    fn reported_ports_override_the_hash_port() {
        let mut reported = peer("b", "10.100.0.3");
        reported.wg_listen_port = Some(50321);
        let peers = build_peers("group", "a", vec![reported, peer("c", "10.100.0.4")], &[]);

        assert_eq!(peers[0].endpoint.as_deref(), Some("192.168.1.5:50321"));
        assert_eq!(
            peers[1].endpoint,
            Some(format!("192.168.1.5:{}", rg_wireguard_port("group")))
        );
    }

    #[test]
    fn routes_cover_only_guests_on_other_nodes() {
        let members = vec![
            member("a", Some("web"), "10.0.0.3"),
            member("b", Some("api"), "10.0.0.4"),
        ];
        let routes = remote_guest_ips("a", &members);

        assert_eq!(routes, HashSet::from(["10.0.0.4".to_string()]));
    }

    #[test]
    fn dns_records_include_every_named_member_regardless_of_node() {
        let members = vec![
            member("a", Some("web"), "10.0.0.3"),
            member("b", Some("api"), "10.0.0.4"),
            member("b", None, "10.0.0.5"),
        ];
        let records = service_records(&members, &[]);

        assert_eq!(records.len(), 2);
        assert_eq!(records["web"], "10.0.0.3");
        assert_eq!(records["api"], "10.0.0.4");
    }

    #[test]
    fn site_services_are_published_and_never_shadow_a_member() {
        let members = vec![member("a", Some("docs"), "10.0.0.3")];
        let sites = vec![
            SiteService {
                service_name: "docs".to_string(),
                address: "192.168.105.200".to_string(),
            },
            SiteService {
                service_name: "blog".to_string(),
                address: "192.168.105.200".to_string(),
            },
        ];
        let records = service_records(&members, &sites);

        assert_eq!(records["blog"], "192.168.105.200");
        assert_eq!(records["docs"], "10.0.0.3");
    }

    #[test]
    fn endpoint_without_a_port_is_rejected() {
        assert_eq!(with_port("no-port", 1000), None);
    }
}
