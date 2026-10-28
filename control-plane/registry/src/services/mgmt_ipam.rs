use anyhow::{Context, Result};
use etcd_client::{Client, Compare, CompareOp, Txn, TxnOp};
use std::net::Ipv4Addr;
use uuid::Uuid;

const MGMT_IPAM_PREFIX: &str = "/csfx/ipam/mgmt/";
const MGMT_CIDR_BASE: Ipv4Addr = Ipv4Addr::new(10, 100, 0, 0);
const MGMT_CIDR_PREFIX_LEN: u32 = 16;

pub struct MgmtIpamService {
    etcd_endpoints: String,
}

impl MgmtIpamService {
    pub fn new(etcd_endpoints: String) -> Self {
        Self { etcd_endpoints }
    }

    pub async fn allocate(&self, agent_id: Uuid) -> Result<String> {
        let mut client = Client::connect([self.etcd_endpoints.as_str()], None)
            .await
            .context("failed to connect to etcd")?;

        let total = 1u32 << (32 - MGMT_CIDR_PREFIX_LEN);

        for offset in 2..total - 1 {
            let candidate = ip_add(MGMT_CIDR_BASE, offset);
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

fn ip_add(base: Ipv4Addr, offset: u32) -> Ipv4Addr {
    let n = u32::from(base) + offset;
    Ipv4Addr::from(n)
}
