use anyhow::{Context, Result};
use etcd_client::{Client, Compare, CompareOp, Txn, TxnOp};
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
    let site_key = format!("{}site/{}", MGMT_SITE_PREFIX, site_id);
    for octet in FIRST_SITE_OCTET..=u8::MAX {
        if let Some(assigned) = assigned_octet(client, &site_key).await? {
            return Ok(block_base(assigned));
        }
        let block_key = format!("{}block/{}", MGMT_SITE_PREFIX, octet);
        let claim = Txn::new()
            .when([
                Compare::create_revision(site_key.as_str(), CompareOp::Equal, 0),
                Compare::create_revision(block_key.as_str(), CompareOp::Equal, 0),
            ])
            .and_then([
                TxnOp::put(site_key.as_str(), octet.to_string(), None),
                TxnOp::put(block_key.as_str(), site_id, None),
            ]);
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

async fn assigned_octet(client: &mut Client, site_key: &str) -> Result<Option<u8>> {
    let response = client
        .get(site_key, None)
        .await
        .context("etcd get failed")?;
    response
        .kvs()
        .first()
        .map(|entry| parse_octet(entry.value_str()?))
        .transpose()
}

fn parse_octet(value: &str) -> Result<u8> {
    value.parse().context("malformed management site octet")
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
    fn site_octet_parses_and_rejects_garbage() {
        assert_eq!(parse_octet("102").unwrap(), 102);
        assert!(parse_octet("256").is_err());
        assert!(parse_octet("").is_err());
    }

    #[tokio::test]
    #[ignore = "requires etcd via ETCD_ENDPOINTS"]
    async fn concurrent_agents_of_a_new_site_share_one_block() {
        let endpoints = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS must be set");
        let service = MgmtIpamService::new(endpoints);
        let site = format!("site-{}", Uuid::new_v4());
        let (first, second) = tokio::join!(
            service.allocate(Uuid::new_v4(), &site),
            service.allocate(Uuid::new_v4(), &site)
        );
        let block = |ip: String| ip.parse::<Ipv4Addr>().unwrap().octets()[1];
        let first = block(first.unwrap());
        assert_ne!(first, DEFAULT_SITE_OCTET);
        assert_eq!(first, block(second.unwrap()));
    }
}
