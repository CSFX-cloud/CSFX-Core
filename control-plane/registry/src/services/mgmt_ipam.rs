use anyhow::{Context, Result};
use etcd_client::{Client, Compare, CompareOp, GetOptions, Txn, TxnOp};
use std::net::Ipv4Addr;
use uuid::Uuid;

const MGMT_IPAM_PREFIX: &str = "/csfx/ipam/mgmt/";
const MGMT_SITE_PREFIX: &str = "/csfx/ipam/mgmt-sites/";
const MGMT_CIDR_PREFIX_LEN: u32 = 16;
const DEFAULT_SITE_OCTET: u8 = 100;
const FIRST_SITE_OCTET: u8 = 101;

pub const DEFAULT_SITE: &str = "default";

pub struct MgmtIpamService {
    etcd_endpoints: String,
}

impl MgmtIpamService {
    pub fn new(etcd_endpoints: String) -> Self {
        Self { etcd_endpoints }
    }

    pub async fn allocate(&self, agent_id: Uuid, site_id: &str) -> Result<String> {
        let mut client = Client::connect([self.etcd_endpoints.as_str()], None)
            .await
            .context("failed to connect to etcd")?;
        let base = site_base(&mut client, site_id).await?;

        let total = 1u32 << (32 - MGMT_CIDR_PREFIX_LEN);

        for offset in 2..total - 1 {
            let candidate = ip_add(base, offset);
            let ip_str = candidate.to_string();
            let key = format!("{}{}", MGMT_IPAM_PREFIX, ip_str);

            let claim = Txn::new()
                .when([Compare::create_revision(key.as_str(), CompareOp::Equal, 0)])
                .and_then([TxnOp::put(key.as_str(), agent_id.to_string(), None)]);
            let resp = client.txn(claim).await.context("etcd txn failed")?;
            if resp.succeeded() {
                return Ok(ip_str);
            }
        }

        anyhow::bail!("management IPAM pool exhausted")
    }

    pub async fn release(&self, ip: &str) -> Result<()> {
        let mut client = Client::connect([self.etcd_endpoints.as_str()], None)
            .await
            .context("failed to connect to etcd")?;

        let key = format!("{}{}", MGMT_IPAM_PREFIX, ip);
        client
            .delete(key.as_str(), None)
            .await
            .context("etcd delete failed")?;
        Ok(())
    }
}

async fn site_base(client: &mut Client, site_id: &str) -> Result<Ipv4Addr> {
    if site_id == DEFAULT_SITE {
        return Ok(block_base(DEFAULT_SITE_OCTET));
    }
    let existing = client
        .get(MGMT_SITE_PREFIX, Some(GetOptions::new().with_prefix()))
        .await
        .context("etcd get failed")?;
    for entry in existing.kvs() {
        if entry.value() == site_id.as_bytes() {
            return octet_from_key(entry.key_str()?).map(block_base);
        }
    }
    for octet in FIRST_SITE_OCTET..=u8::MAX {
        let key = format!("{}{}", MGMT_SITE_PREFIX, octet);
        let claim = Txn::new()
            .when([Compare::create_revision(key.as_str(), CompareOp::Equal, 0)])
            .and_then([TxnOp::put(key.as_str(), site_id, None)]);
        if client
            .txn(claim)
            .await
            .context("etcd txn failed")?
            .succeeded()
        {
            return Ok(block_base(octet));
        }
    }
    anyhow::bail!("management site block pool exhausted")
}

fn octet_from_key(key: &str) -> Result<u8> {
    key.strip_prefix(MGMT_SITE_PREFIX)
        .and_then(|octet| octet.parse().ok())
        .context("malformed management site key")
}

fn block_base(octet: u8) -> Ipv4Addr {
    Ipv4Addr::new(10, octet, 0, 0)
}

fn ip_add(base: Ipv4Addr, offset: u32) -> Ipv4Addr {
    let n = u32::from(base) + offset;
    Ipv4Addr::from(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_site_keeps_the_legacy_block() {
        assert_eq!(block_base(DEFAULT_SITE_OCTET), Ipv4Addr::new(10, 100, 0, 0));
    }

    #[test]
    fn site_key_round_trips_to_its_octet() {
        let key = format!("{}{}", MGMT_SITE_PREFIX, 102);
        assert_eq!(octet_from_key(&key).unwrap(), 102);
        assert!(octet_from_key("/other/102").is_err());
    }
}
