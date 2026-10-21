use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::fs;
use tokio::sync::Mutex;
use tracing::info;

const DNS_DIR: &str = "/var/lib/csfx-agent/dns";
const ZONE_TTL_SECS: u32 = 30;
const ZONE_SERIAL_BASE: u32 = 1;
const DEFAULT_LOCAL_ZONE: &str = "csfx.internal";
const FORWARD_CACHE_SECS: u32 = 10;

fn zone_name(resource_group_id: &str) -> String {
    format!("svc.{}.internal", resource_group_id)
}

fn zone_file_path(resource_group_id: &str) -> PathBuf {
    PathBuf::from(DNS_DIR).join(format!("{}.zone", resource_group_id))
}

pub fn corefile_path(resource_group_id: &str) -> PathBuf {
    PathBuf::from(DNS_DIR).join(format!("{}.Corefile", resource_group_id))
}

pub struct RgDnsRegistry {
    records: Mutex<HashMap<String, HashMap<String, String>>>,
}

impl RgDnsRegistry {
    pub fn new() -> Self {
        Self {
            records: Mutex::new(HashMap::new()),
        }
    }

    pub async fn upsert(
        &self,
        resource_group_id: &str,
        service_name: &str,
        ip_address: &str,
    ) -> Result<()> {
        let mut records = self.records.lock().await;
        let rg_records = records.entry(resource_group_id.to_string()).or_default();
        rg_records.insert(service_name.to_string(), ip_address.to_string());
        let snapshot = rg_records.clone();
        drop(records);

        write_zone_file(resource_group_id, &snapshot).await
    }

    pub async fn remove(&self, resource_group_id: &str, service_name: &str) -> Result<()> {
        let mut records = self.records.lock().await;
        let snapshot = match records.get_mut(resource_group_id) {
            Some(rg_records) => {
                rg_records.remove(service_name);
                rg_records.clone()
            }
            None => return Ok(()),
        };
        drop(records);

        write_zone_file(resource_group_id, &snapshot).await
    }
}

async fn write_zone_file(resource_group_id: &str, records: &HashMap<String, String>) -> Result<()> {
    fs::create_dir_all(DNS_DIR)
        .await
        .context("Failed to create dns zone directory")?;

    let zone = zone_name(resource_group_id);
    let mut contents = format!(
        "$TTL {ttl}\n@ IN SOA ns.{zone}. admin.{zone}. ( {serial} {ttl} {ttl} {ttl} {ttl} )\n@ IN NS ns.{zone}.\nns IN A 127.0.0.1\n",
        ttl = ZONE_TTL_SECS,
        zone = zone,
        serial = ZONE_SERIAL_BASE,
    );

    for (service_name, ip_address) in records {
        contents.push_str(&format!("{} IN A {}\n", service_name, ip_address));
    }

    let path = zone_file_path(resource_group_id);
    fs::write(&path, contents)
        .await
        .with_context(|| format!("Failed to write dns zone file {:?}", path))?;

    info!(resource_group_id = %resource_group_id, records = records.len(), "Resource group dns zone updated");
    Ok(())
}

async fn ensure_zone_file(resource_group_id: &str) -> Result<()> {
    let path = zone_file_path(resource_group_id);
    if fs::try_exists(&path).await.unwrap_or(false) {
        return Ok(());
    }
    write_zone_file(resource_group_id, &HashMap::new()).await
}

struct LocalForward {
    zone: String,
    servers: String,
}

fn local_forward_from_env() -> Option<LocalForward> {
    let servers = std::env::var("CSFX_LOCAL_DNS_SERVERS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|value| !value.is_empty())?;
    let zone = std::env::var("CSFX_LOCAL_ZONE")
        .ok()
        .map(|value| value.trim().trim_matches('.').to_ascii_lowercase())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_LOCAL_ZONE.to_string());
    Some(LocalForward { zone, servers })
}

fn corefile_contents(
    zone: &str,
    listen_ip: &str,
    zone_path: &str,
    forward: Option<&LocalForward>,
) -> String {
    let mut contents = format!(
        "{zone}:53 {{\n    bind {listen_ip}\n    file {zone_path}\n    reload 2s\n    log\n    errors\n}}\n"
    );
    if let Some(forward) = forward {
        contents.push_str(&format!(
            "{}:53 {{\n    bind {listen_ip}\n    forward . {}\n    cache {FORWARD_CACHE_SECS}\n    errors\n}}\n",
            forward.zone, forward.servers
        ));
    }
    contents
}

pub async fn write_corefile(resource_group_id: &str, listen_ip: &str) -> Result<()> {
    fs::create_dir_all(DNS_DIR)
        .await
        .context("Failed to create dns zone directory")?;

    ensure_zone_file(resource_group_id).await?;

    let zone_path = zone_file_path(resource_group_id).display().to_string();
    let contents = corefile_contents(
        &zone_name(resource_group_id),
        listen_ip,
        &zone_path,
        local_forward_from_env().as_ref(),
    );

    let path = corefile_path(resource_group_id);
    fs::write(&path, contents)
        .await
        .with_context(|| format!("Failed to write corefile {:?}", path))?;

    Ok(())
}

pub async fn remove_zone_files(resource_group_id: &str) -> Result<()> {
    let zone_path = zone_file_path(resource_group_id);
    let corefile = corefile_path(resource_group_id);

    let _ = fs::remove_file(&zone_path).await;
    let _ = fs::remove_file(&corefile).await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corefile_without_forward_serves_only_the_group_zone() {
        let contents = corefile_contents("svc.rg.internal", "10.0.0.2", "/z/rg.zone", None);
        assert!(contents.contains("svc.rg.internal:53"));
        assert!(contents.contains("file /z/rg.zone"));
        assert!(!contents.contains("forward"));
    }

    #[test]
    fn corefile_with_forward_adds_the_platform_zone_block() {
        let forward = LocalForward {
            zone: "csfx.internal".to_string(),
            servers: "192.168.1.10 192.168.1.11".to_string(),
        };
        let contents =
            corefile_contents("svc.rg.internal", "10.0.0.2", "/z/rg.zone", Some(&forward));
        assert!(contents.contains(
            "csfx.internal:53 {\n    bind 10.0.0.2\n    forward . 192.168.1.10 192.168.1.11"
        ));
        assert_eq!(contents.matches("bind 10.0.0.2").count(), 2);
    }
}
