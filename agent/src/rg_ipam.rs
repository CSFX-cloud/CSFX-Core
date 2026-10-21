use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::spec::{cidr_host_count, nth_host_ip};

const IPAM_REGISTRY_DIR: &str = "/var/lib/csfx-agent/rg-ipam";
const FIRST_GUEST_OFFSET: u32 = 3;

pub async fn allocate(resource_group_id: &str, cidr: &str, workload_id: &str) -> Result<String> {
    let rg_dir = registry_dir(resource_group_id);
    tokio::fs::create_dir_all(&rg_dir)
        .await
        .context("failed to create rg ipam registry directory")?;

    if let Some(existing) = find_existing(&rg_dir, workload_id).await? {
        return Ok(existing);
    }

    let host_count = cidr_host_count(cidr).context("invalid resource group cidr")?;
    let usable_hosts = host_count.saturating_sub(FIRST_GUEST_OFFSET + 1);

    for slot in 0..usable_hosts {
        let offset = FIRST_GUEST_OFFSET + slot;
        let Some(ip) = nth_host_ip(cidr, offset) else {
            continue;
        };

        let claim_path = rg_dir.join(&ip);
        match tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&claim_path)
            .await
        {
            Ok(mut file) => {
                use tokio::io::AsyncWriteExt;
                file.write_all(workload_id.as_bytes())
                    .await
                    .context("failed to write ipam claim")?;
                return Ok(ip);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e).context("failed to claim ipam slot"),
        }
    }

    anyhow::bail!(
        "no free ip addresses available in resource group {} cidr {}",
        resource_group_id,
        cidr
    )
}

pub async fn is_claimed(resource_group_id: &str, ip: &str) -> bool {
    tokio::fs::try_exists(registry_dir(resource_group_id).join(ip))
        .await
        .unwrap_or(false)
}

pub async fn release(resource_group_id: &str, workload_id: &str) {
    let rg_dir = registry_dir(resource_group_id);
    let Ok(mut entries) = tokio::fs::read_dir(&rg_dir).await else {
        return;
    };

    while let Ok(Some(entry)) = entries.next_entry().await {
        if let Ok(claimed_by) = tokio::fs::read_to_string(entry.path()).await {
            if claimed_by == workload_id {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}

async fn find_existing(rg_dir: &Path, workload_id: &str) -> Result<Option<String>> {
    let mut entries = match tokio::fs::read_dir(rg_dir).await {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).context("failed to read rg ipam registry"),
    };

    while let Some(entry) = entries
        .next_entry()
        .await
        .context("failed to read rg ipam registry entry")?
    {
        if let Ok(claimed_by) = tokio::fs::read_to_string(entry.path()).await {
            if claimed_by == workload_id {
                if let Some(ip) = entry.file_name().to_str() {
                    return Ok(Some(ip.to_string()));
                }
            }
        }
    }

    Ok(None)
}

fn registry_dir(resource_group_id: &str) -> PathBuf {
    Path::new(IPAM_REGISTRY_DIR).join(resource_group_id)
}
