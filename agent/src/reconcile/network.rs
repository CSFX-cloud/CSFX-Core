use std::collections::HashMap;
use tracing::{info, warn};

use super::Reconciler;
use crate::{client, rg_network, wireguard};

impl Reconciler {
    pub(super) async fn prepare_networks(&self, workloads: &[client::AssignedWorkload]) {
        self.ensure_group_dns(workloads).await;
        rg_network::record_cidrs(&assigned_ranges(workloads)).await;
    }

    pub(super) async fn sync_mgmt_peers(&self, resource_group_ids: &[String]) {
        let Some(mut wg_peers) = self.fetch_mesh_wg_peers().await else {
            return;
        };
        wg_peers.extend(self.fetch_vpn_wg_peers(resource_group_ids).await);

        if let Err(e) = wireguard::apply_mgmt_peers(&wg_peers).await {
            warn!(error = %e, "Failed to sync management WireGuard peers");
            return;
        }

        if let Err(e) = wireguard::save_peer_cache(&wg_peers) {
            warn!(error = %e, "Failed to cache management WireGuard peers");
        }
    }

    pub(super) async fn cleanup_stale_resource_groups(&self) {
        let active_ids = match self.client.fetch_active_resource_group_ids().await {
            Ok(ids) => ids,
            Err(e) => {
                warn!(error = %e, "Failed to fetch active resource group ids");
                return;
            }
        };

        let local_ids = match rg_network::list_rg_ids().await {
            Ok(ids) => ids,
            Err(e) => {
                warn!(error = %e, "Failed to list local resource group bridges");
                return;
            }
        };

        for local_id in local_ids.iter().filter(|id| !active_ids.contains(id)) {
            self.teardown_resource_group(local_id).await;
        }
    }

    async fn teardown_resource_group(&self, resource_group_id: &str) {
        info!(resource_group_id = %resource_group_id, "Tearing down stale resource group network");

        if let Err(e) = self
            .runtimes
            .firecracker
            .teardown_rg_network(resource_group_id)
            .await
        {
            warn!(resource_group_id = %resource_group_id, error = %e, "Failed to tear down stale firecracker resource group network");
        }
        if let Err(e) = self
            .runtimes
            .qemu
            .teardown_rg_network(resource_group_id)
            .await
        {
            warn!(resource_group_id = %resource_group_id, error = %e, "Failed to tear down stale qemu resource group network");
        }
    }

    async fn ensure_group_dns(&self, workloads: &[client::AssignedWorkload]) {
        let groups: HashMap<&str, &str> = workloads
            .iter()
            .filter_map(|workload| {
                Some((
                    workload.resource_group_id.as_deref()?,
                    workload.resource_group_cidr.as_deref()?,
                ))
            })
            .collect();
        for (resource_group_id, cidr) in groups {
            if let Err(e) = self
                .runtimes
                .firecracker
                .ensure_group_dns(resource_group_id, cidr)
                .await
            {
                warn!(resource_group_id = %resource_group_id, error = %e, "Failed to ensure resource group dns process");
            }
        }
    }

    async fn fetch_mesh_wg_peers(&self) -> Option<Vec<wireguard::Peer>> {
        let peers = match self.client.fetch_mesh_peers().await {
            Ok(peers) => peers,
            Err(e) => {
                warn!(error = %e, "Failed to fetch mesh peers");
                return None;
            }
        };

        let agent_id = self.agent_id.to_string();
        Some(
            peers
                .into_iter()
                .filter(|p| p.agent_id != agent_id)
                .map(|p| wireguard::Peer {
                    public_key: p.wg_public_key,
                    endpoint: Some(p.wg_endpoint),
                    allowed_ips: format!("{}/32", p.wg_tunnel_ip),
                })
                .collect(),
        )
    }

    async fn fetch_vpn_wg_peers(&self, resource_group_ids: &[String]) -> Vec<wireguard::Peer> {
        let mut wg_peers = Vec::new();

        for resource_group_id in resource_group_ids {
            match self
                .client
                .fetch_resource_group_vpn_peers(resource_group_id)
                .await
            {
                Ok(peers) => {
                    wg_peers.extend(peers.into_iter().map(|p| wireguard::Peer {
                        public_key: p.client_public_key,
                        endpoint: None,
                        allowed_ips: format!("{}/32", p.client_tunnel_ip),
                    }));
                }
                Err(e) => {
                    warn!(resource_group_id = %resource_group_id, error = %e, "Failed to fetch resource group vpn peers");
                }
            }
        }

        wg_peers
    }
}

fn assigned_ranges(workloads: &[client::AssignedWorkload]) -> Vec<(String, String)> {
    let mut ranges: Vec<(String, String)> = workloads
        .iter()
        .filter_map(|w| Some((w.resource_group_id.clone()?, w.resource_group_cidr.clone()?)))
        .collect();
    ranges.sort_unstable();
    ranges.dedup();
    ranges
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::workload;
    use super::*;

    #[test]
    fn assigned_ranges_are_unique_and_skip_groups_without_a_range() {
        let mut first = workload("w1", "always", None);
        first.resource_group_id = Some("rg-1".to_string());
        first.resource_group_cidr = Some("10.101.0.0/24".to_string());
        let mut second = first.clone();
        second.id = "w2".to_string();
        let mut without_range = workload("w3", "always", None);
        without_range.resource_group_id = Some("rg-2".to_string());

        let ranges =
            assigned_ranges(&[first, second, without_range, workload("w4", "always", None)]);

        assert_eq!(
            ranges,
            vec![("rg-1".to_string(), "10.101.0.0/24".to_string())]
        );
    }
}
