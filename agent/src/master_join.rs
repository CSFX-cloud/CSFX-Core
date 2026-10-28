use anyhow::{bail, Context, Result};
use std::fs::{OpenOptions, Permissions};
use std::io::Write;
use std::os::unix::fs::{chown, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::client::{ApiClient, ClusterVip, MasterBundle};
use crate::{config, pki};

const PENDING_MARKER: &str = "/etc/csfx/master-join-pending";
const PROMOTE_MARKER: &str = "/etc/csfx/master-join-promote";
const SETUP_DONE_FILE: &str = "/var/lib/csfx-data/csfx/.setup-complete";
const CA_CERT_PATH: &str = "/var/lib/csfx-cp/ca.crt";
const CA_KEY_PATH: &str = "/var/lib/csfx-cp/ca.key";
const CP_ENV_PATH: &str = "/etc/csfx/cp.env";
const GARAGE_ENV_PATH: &str = "/etc/csfx/garage.env";
pub(crate) const CLUSTER_ENV_PATH: &str = "/etc/csfx/cluster.env";
const ETCD_JOIN_ENV_PATH: &str = "/etc/csfx/etcd-join.env";
const PATRONI_SUPERUSER_PATH: &str = "/etc/csfx/patroni-superuser-password";
const PATRONI_REPLICATION_PATH: &str = "/etc/csfx/patroni-replication-password";
const PATRONI_GROUP: &str = "patroni";
const ETCD_DATA_DIR: &str = "/var/lib/etcd";
const LOCAL_STATE_DIRS: [&str; 6] = [
    ETCD_DATA_DIR,
    "/var/lib/patroni",
    "/var/lib/postgresql",
    "/var/lib/csfx-cp",
    "/var/lib/csfx-garage/meta",
    "/var/lib/csfx-garage/data",
];
const AGENT_JOIN_ENV_PATH: &str = "/etc/csfx/agent-join.env";
const GARAGE_HA_REPLICATION_FACTOR: u32 = 3;
const AGENT_UNIT: &str = "csfx-agent.service";
const ETCD_UNIT: &str = "etcd.service";
const CONTROL_PLANE_TARGET: &str = "csfx-cp.target";
const POLL_INTERVAL: Duration = Duration::from_secs(5);
const REGISTRATION_ATTEMPTS: u32 = 120;
const PROMOTE_ATTEMPTS: u32 = 180;

pub async fn run(gateway_url: &str) -> Result<()> {
    if Path::new(PENDING_MARKER).exists() {
        join(gateway_url).await?;
    }
    if Path::new(PROMOTE_MARKER).exists() {
        promote(gateway_url).await?;
    }
    Ok(())
}

pub(crate) async fn authenticated_client(gateway_url: &str) -> Result<(ApiClient, String)> {
    wait_for_registration().await?;
    let credentials = config::load_credentials().context("Failed to load credentials")?;
    let ca_pem = pki::AgentPki::load_ca_pem().context("Failed to load cluster CA")?;
    let client = ApiClient::new(
        gateway_url.to_string(),
        String::new(),
        None,
        None,
        Some(&ca_pem),
    )?;
    Ok((client, credentials.api_key))
}

async fn wait_for_registration() -> Result<()> {
    for _ in 0..REGISTRATION_ATTEMPTS {
        if config::is_registered() && pki::AgentPki::load_ca_pem().is_ok() {
            return Ok(());
        }
        sleep(POLL_INTERVAL).await;
    }
    bail!("agent did not register in time")
}

async fn join(gateway_url: &str) -> Result<()> {
    let (client, api_key) = authenticated_client(gateway_url).await?;
    let bundle = client.request_master_bundle(&api_key).await?;
    reset_local_control_plane()?;
    apply_bundle(&bundle).await?;
    write_file(PROMOTE_MARKER, "", 0o600)?;
    std::fs::remove_file(PENDING_MARKER).context("Failed to remove join marker")
}

async fn promote(gateway_url: &str) -> Result<()> {
    let (client, api_key) = authenticated_client(gateway_url).await?;
    start_fresh_etcd().await?;
    wait_for_promotion(&client, &api_key).await?;
    info!("etcd member promoted to voter");
    start_unit(CONTROL_PLANE_TARGET).await?;
    std::fs::remove_file(PROMOTE_MARKER).context("Failed to remove promote marker")?;
    std::fs::remove_file(AGENT_JOIN_ENV_PATH).context("Failed to remove agent join env")?;
    systemctl("restart", AGENT_UNIT).await
}

async fn start_fresh_etcd() -> Result<()> {
    systemctl("stop", ETCD_UNIT).await?;
    clear_directory(ETCD_DATA_DIR)?;
    start_unit(ETCD_UNIT).await
}

async fn wait_for_promotion(client: &ApiClient, api_key: &str) -> Result<()> {
    for _ in 0..PROMOTE_ATTEMPTS {
        if client.promote_master(api_key).await? {
            return Ok(());
        }
        sleep(POLL_INTERVAL).await;
    }
    bail!("etcd learner did not catch up in time")
}

fn reset_local_control_plane() -> Result<()> {
    for dir in LOCAL_STATE_DIRS {
        clear_directory(dir)?;
    }
    info!("discarded local control plane state of the previous standalone install");
    Ok(())
}

fn clear_directory(path: &str) -> Result<()> {
    let entries = match std::fs::read_dir(path) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err).with_context(|| format!("Failed to read {path}")),
    };
    for entry in entries {
        let entry_path = entry
            .with_context(|| format!("Failed to list {path}"))?
            .path();
        let removed = if entry_path.is_dir() {
            std::fs::remove_dir_all(&entry_path)
        } else {
            std::fs::remove_file(&entry_path)
        };
        removed.with_context(|| format!("Failed to remove {}", entry_path.display()))?;
    }
    Ok(())
}

async fn apply_bundle(bundle: &MasterBundle) -> Result<()> {
    write_file(CA_CERT_PATH, &bundle.ca_cert_pem, 0o644)?;
    write_file(CA_KEY_PATH, &bundle.ca_key_pem, 0o600)?;
    adopt_directory_owner(&[CA_CERT_PATH, CA_KEY_PATH])?;
    write_file(CP_ENV_PATH, &render_cp_env(bundle), 0o600)?;
    write_file(GARAGE_ENV_PATH, &render_garage_env(bundle), 0o600)?;
    write_file(ETCD_JOIN_ENV_PATH, &render_etcd_env(bundle), 0o600)?;
    apply_vip(bundle.vip.as_ref())?;
    write_file(
        PATRONI_SUPERUSER_PATH,
        &bundle.patroni_superuser_password,
        0o640,
    )?;
    write_file(
        PATRONI_REPLICATION_PATH,
        &bundle.patroni_replication_password,
        0o640,
    )?;
    grant_group(
        PATRONI_GROUP,
        &[PATRONI_SUPERUSER_PATH, PATRONI_REPLICATION_PATH],
    )
    .await?;
    write_file(SETUP_DONE_FILE, "", 0o600)
}

fn apply_vip(vip: Option<&ClusterVip>) -> Result<()> {
    match vip {
        Some(vip) => write_file(CLUSTER_ENV_PATH, &render_cluster_env(vip), 0o644),
        None => Ok(()),
    }
}

fn adopt_directory_owner(paths: &[&str]) -> Result<()> {
    for path in paths {
        let Some(parent) = Path::new(path).parent() else {
            continue;
        };
        let owner = std::fs::metadata(parent)
            .with_context(|| format!("Failed to stat {}", parent.display()))?;
        chown(path, Some(owner.uid()), Some(owner.gid()))
            .with_context(|| format!("Failed to change owner of {path}"))?;
    }
    Ok(())
}

async fn grant_group(group: &str, paths: &[&str]) -> Result<()> {
    let status = Command::new("chgrp")
        .arg(group)
        .args(paths)
        .status()
        .await
        .context("Failed to run chgrp")?;
    if !status.success() {
        bail!("chgrp failed group={group}");
    }
    Ok(())
}

async fn start_unit(unit: &str) -> Result<()> {
    systemctl("start", unit).await
}

pub(crate) async fn systemctl(action: &str, unit: &str) -> Result<()> {
    let status = Command::new("systemctl")
        .arg(action)
        .arg(unit)
        .status()
        .await
        .context("Failed to run systemctl")?;
    if !status.success() {
        warn!(action, unit, "systemctl action failed");
        bail!("systemctl {action} failed unit={unit}");
    }
    Ok(())
}

pub(crate) fn write_file(path: &str, contents: &str, mode: u32) -> Result<()> {
    if let Some(parent) = Path::new(path).parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create {path} parent"))?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)
        .with_context(|| format!("Failed to open {path}"))?;
    file.write_all(contents.as_bytes())
        .with_context(|| format!("Failed to write {path}"))?;
    std::fs::set_permissions(path, Permissions::from_mode(mode))
        .with_context(|| format!("Failed to set permissions on {path}"))
}

fn render_cp_env(bundle: &MasterBundle) -> String {
    format!(
        "DATABASE_URL=postgres://csfx:{}@localhost:5432/csfx\nJWT_SECRET={}\nETCD_ENDPOINTS=http://localhost:2379\nGARAGE_ADMIN_TOKEN={}\nOBJECT_STORAGE_ENCRYPTION_KEY={}\n",
        bundle.db_password,
        bundle.jwt_secret,
        bundle.garage_admin_token,
        bundle.object_storage_encryption_key
    )
}

fn render_garage_env(bundle: &MasterBundle) -> String {
    let replication = bundle.vip.as_ref().map_or_else(String::new, |_| {
        format!("GARAGE_REPLICATION_FACTOR={GARAGE_HA_REPLICATION_FACTOR}\n")
    });
    format!(
        "GARAGE_RPC_SECRET={}\nGARAGE_ADMIN_TOKEN={}\n{replication}",
        bundle.garage_rpc_secret, bundle.garage_admin_token
    )
}

pub(crate) fn render_cluster_env(vip: &ClusterVip) -> String {
    format!(
        "CSFX_VIP_ADDRESS={}\nCSFX_VIP_PREFIX={}\nCSFX_VIP_ROUTER_ID={}\nTLS_SANS=localhost,127.0.0.1,{}\nCSFX_LOCAL_DNS_ADDRESSES={}\n",
        vip.address, vip.prefix_length, vip.router_id, vip.address, vip.address
    )
}

fn render_etcd_env(bundle: &MasterBundle) -> String {
    format!(
        "ETCD_NAME={}\nETCD_INITIAL_CLUSTER={}\nETCD_INITIAL_CLUSTER_STATE=existing\n",
        bundle.etcd_name, bundle.etcd_initial_cluster
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bundle() -> MasterBundle {
        MasterBundle {
            ca_cert_pem: "cert".into(),
            ca_key_pem: "key".into(),
            jwt_secret: "jwt".into(),
            garage_admin_token: "admin".into(),
            object_storage_encryption_key: "enc".into(),
            db_password: "dbpw".into(),
            garage_rpc_secret: "rpc".into(),
            patroni_superuser_password: "super".into(),
            patroni_replication_password: "repl".into(),
            etcd_name: "node-b".into(),
            etcd_initial_cluster: "node-a=http://10.100.0.1:2380,node-b=http://10.100.0.2:2380"
                .into(),
            vip: None,
        }
    }

    #[test]
    fn cp_env_matches_setup_script_layout() {
        let env = render_cp_env(&bundle());
        assert!(env.contains("DATABASE_URL=postgres://csfx:dbpw@localhost:5432/csfx\n"));
        assert!(env.contains("JWT_SECRET=jwt\n"));
        assert!(env.contains("ETCD_ENDPOINTS=http://localhost:2379\n"));
        assert!(env.contains("OBJECT_STORAGE_ENCRYPTION_KEY=enc\n"));
    }

    #[test]
    fn etcd_env_joins_existing_cluster() {
        let env = render_etcd_env(&bundle());
        assert!(env.contains("ETCD_NAME=node-b\n"));
        assert!(env.contains("ETCD_INITIAL_CLUSTER_STATE=existing\n"));
        assert!(env.contains(
            "ETCD_INITIAL_CLUSTER=node-a=http://10.100.0.1:2380,node-b=http://10.100.0.2:2380\n"
        ));
    }

    #[test]
    fn cluster_env_carries_vip_and_tls_names() {
        let vip = ClusterVip {
            address: "192.168.1.50".into(),
            prefix_length: 24,
            router_id: 51,
        };
        assert_eq!(
            render_cluster_env(&vip),
            "CSFX_VIP_ADDRESS=192.168.1.50\nCSFX_VIP_PREFIX=24\nCSFX_VIP_ROUTER_ID=51\nTLS_SANS=localhost,127.0.0.1,192.168.1.50\nCSFX_LOCAL_DNS_ADDRESSES=192.168.1.50\n"
        );
    }

    #[test]
    fn garage_env_requests_full_replication_behind_a_vip() {
        let mut bundle = bundle();
        bundle.vip = Some(ClusterVip {
            address: "192.168.1.50".into(),
            prefix_length: 24,
            router_id: 51,
        });
        assert!(render_garage_env(&bundle).ends_with("GARAGE_REPLICATION_FACTOR=3\n"));
    }

    #[test]
    fn garage_env_carries_rpc_secret() {
        assert_eq!(
            render_garage_env(&bundle()),
            "GARAGE_RPC_SECRET=rpc\nGARAGE_ADMIN_TOKEN=admin\n"
        );
    }
}
