use anyhow::Result;
use tracing::info;

use crate::master_join::{
    authenticated_client, render_cluster_env, systemctl, write_file, CLUSTER_ENV_PATH,
};

const RESTARTED_UNITS: [&str; 3] = [
    "csfx-keepalived.service",
    "csfx-registry.service",
    "csfx-api-gateway.service",
];

pub async fn run(gateway_url: &str) -> Result<()> {
    let (client, api_key) = authenticated_client(gateway_url).await?;
    let Some(vip) = client.fetch_cluster_vip(&api_key).await? else {
        return Ok(());
    };
    let rendered = render_cluster_env(&vip);
    if std::fs::read_to_string(CLUSTER_ENV_PATH).is_ok_and(|current| current == rendered) {
        return Ok(());
    }
    write_file(CLUSTER_ENV_PATH, &rendered, 0o644)?;
    info!(address = %vip.address, prefix_length = vip.prefix_length, router_id = vip.router_id, "cluster vip applied");
    for unit in RESTARTED_UNITS {
        systemctl("restart", unit).await?;
    }
    Ok(())
}
