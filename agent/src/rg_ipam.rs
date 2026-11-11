use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::spec::{cidr_host_count, nth_host_ip};

const IPAM_REGISTRY_DIR: &str = "/var/lib/csfx-agent/rg-ipam";
const FIRST_GUEST_OFFSET: u32 = 3;

pub async fn allocate(
    resource_group_id: &str,
    cidr: &str,
    workload_id: &str,
    assigned_ip: Option<&str>,
) -> Result<String> {
    allocate_in(
        &registry_dir(resource_group_id),
        resource_group_id,
        cidr,
        workload_id,
        assigned_ip,
    )
    .await
}

async fn allocate_in(
    rg_dir: &Path,
    resource_group_id: &str,
    cidr: &str,
    workload_id: &str,
    assigned_ip: Option<&str>,
) -> Result<String> {
    tokio::fs::create_dir_all(rg_dir)
        .await
        .context("failed to create rg ipam registry directory")?;

    if let Some(assigned_ip) = assigned_ip {
        return claim_assigned(rg_dir, workload_id, assigned_ip).await;
    }

    if let Some(existing) = find_existing(rg_dir, workload_id).await? {
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

async fn claim_assigned(rg_dir: &Path, workload_id: &str, assigned_ip: &str) -> Result<String> {
    while let Some(existing) = find_existing(rg_dir, workload_id).await? {
        if existing == assigned_ip {
            return Ok(existing);
        }
        tokio::fs::remove_file(rg_dir.join(&existing))
            .await
            .context("failed to drop stale ipam claim")?;
    }

    tokio::fs::write(rg_dir.join(assigned_ip), workload_id.as_bytes())
        .await
        .context("failed to write assigned ipam claim")?;
    Ok(assigned_ip.to_string())
}

pub async fn is_claimed(resource_group_id: &str, ip: &str) -> bool {
    tokio::fs::try_exists(registry_dir(resource_group_id).join(ip))
        .await
        .unwrap_or(false)
}

pub async fn release(resource_group_id: &str, workload_id: &str) {
    release_in(&registry_dir(resource_group_id), workload_id).await;
}

async fn release_in(rg_dir: &Path, workload_id: &str) {
    let Ok(mut entries) = tokio::fs::read_dir(rg_dir).await else {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir() -> PathBuf {
        std::env::temp_dir().join(format!("csfx-ipam-test-{}", uuid::Uuid::new_v4()))
    }

    async fn allocate_for(
        dir: &Path,
        cidr: &str,
        workload: &str,
        fixed: Option<&str>,
    ) -> Result<String> {
        allocate_in(dir, "rg", cidr, workload, fixed).await
    }

    #[tokio::test]
    async fn addresses_start_after_the_gateway_and_are_stable() {
        let dir = temp_dir();
        let first = allocate_for(&dir, "10.0.0.0/24", "a", None).await.unwrap();
        let second = allocate_for(&dir, "10.0.0.0/24", "b", None).await.unwrap();
        let again = allocate_for(&dir, "10.0.0.0/24", "a", None).await.unwrap();
        assert_eq!(first, "10.0.0.3");
        assert_eq!(second, "10.0.0.4");
        assert_eq!(again, first);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn exhausted_and_invalid_ranges_fail() {
        let dir = temp_dir();
        let full = allocate_for(&dir, "10.0.0.0/30", "a", None).await;
        assert!(full.unwrap_err().to_string().contains("no free ip"));
        assert!(allocate_for(&dir, "bogus", "d", None).await.is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn assigned_addresses_replace_stale_claims() {
        let dir = temp_dir();
        let dynamic = allocate_for(&dir, "10.0.0.0/24", "a", None).await.unwrap();
        let fixed = allocate_for(&dir, "10.0.0.0/24", "a", Some("10.0.0.50"))
            .await
            .unwrap();
        assert_eq!(fixed, "10.0.0.50");
        assert!(!dir.join(&dynamic).exists());
        let same = allocate_for(&dir, "10.0.0.0/24", "a", Some("10.0.0.50"))
            .await
            .unwrap();
        assert_eq!(same, fixed);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn release_frees_only_the_workload_claims() {
        let dir = temp_dir();
        allocate_for(&dir, "10.0.0.0/24", "a", None).await.unwrap();
        allocate_for(&dir, "10.0.0.0/24", "b", None).await.unwrap();
        release_in(&dir, "a").await;
        assert!(!dir.join("10.0.0.3").exists());
        assert!(dir.join("10.0.0.4").exists());
        release_in(&dir.join("missing"), "a").await;
        assert_eq!(
            allocate_for(&dir, "10.0.0.0/24", "c", None).await.unwrap(),
            "10.0.0.3"
        );
        assert!(find_existing(&dir.join("missing"), "a")
            .await
            .unwrap()
            .is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[tokio::test]
    async fn unknown_groups_have_no_claims() {
        assert!(!is_claimed("rg-that-does-not-exist", "10.0.0.3").await);
        release("rg-that-does-not-exist", "w").await;
    }
}
