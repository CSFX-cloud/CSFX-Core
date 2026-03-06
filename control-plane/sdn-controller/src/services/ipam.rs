use anyhow::{Context, Result};
use etcd_client::Client;
use std::net::Ipv4Addr;
use uuid::Uuid;

const IPAM_PREFIX: &str = "/csf/ipam/";

#[derive(Clone)]
pub struct IpamService {
    etcd: Client,
}

impl IpamService {
    pub fn new(etcd: Client) -> Self {
        Self { etcd }
    }

    pub async fn allocate(&mut self, network_id: Uuid, cidr: &str, workload_id: Uuid) -> Result<String> {
        let (base, prefix_len) = parse_cidr(cidr)?;
        let total = 1u32 << (32 - prefix_len);

        for offset in 2..total - 1 {
            let candidate = ip_add(base, offset);
            let ip_str = format!("{}", candidate);
            let key = format!("{}{}/{}", IPAM_PREFIX, network_id, ip_str);

            let resp = self
                .etcd
                .get(key.as_str(), None)
                .await
                .context("etcd get failed")?;

            if resp.kvs().is_empty() {
                self.etcd
                    .put(key.as_str(), workload_id.to_string(), None)
                    .await
                    .context("etcd put failed")?;
                return Ok(ip_str);
            }
        }

        anyhow::bail!("IPAM pool exhausted for network={}", network_id)
    }

    pub async fn release(&mut self, network_id: Uuid, ip: &str) -> Result<()> {
        let key = format!("{}{}/{}", IPAM_PREFIX, network_id, ip);
        self.etcd
            .delete(key.as_str(), None)
            .await
            .context("etcd delete failed")?;
        Ok(())
    }

    pub async fn store_peer(
        &mut self,
        network_id: Uuid,
        node_id: &str,
        overlay_ip: &str,
        public_key: Option<&str>,
    ) -> Result<()> {
        let base_key = format!("/csf/peers/{}/{}", network_id, node_id);
        self.etcd
            .put(format!("{}/overlay_ip", base_key).as_str(), overlay_ip, None)
            .await
            .context("etcd put overlay_ip failed")?;

        if let Some(key) = public_key {
            self.etcd
                .put(format!("{}/pubkey", base_key).as_str(), key, None)
                .await
                .context("etcd put pubkey failed")?;
        }

        Ok(())
    }

    pub async fn remove_peer(&mut self, network_id: Uuid, node_id: &str) -> Result<()> {
        let prefix = format!("/csf/peers/{}/{}", network_id, node_id);
        self.etcd
            .delete(
                prefix.as_str(),
                Some(etcd_client::DeleteOptions::new().with_prefix()),
            )
            .await
            .context("etcd delete peer failed")?;
        Ok(())
    }
}

fn parse_cidr(cidr: &str) -> Result<(Ipv4Addr, u32)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid CIDR: {}", cidr);
    }
    let addr: Ipv4Addr = parts[0].parse().context("Invalid IP address")?;
    let prefix: u32 = parts[1].parse().context("Invalid prefix length")?;
    Ok((addr, prefix))
}

fn ip_add(base: Ipv4Addr, offset: u32) -> Ipv4Addr {
    let n = u32::from(base) + offset;
    Ipv4Addr::from(n)
}
