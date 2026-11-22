use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use tracing::{info, warn};

const GRACE_PERIOD: Duration = Duration::from_secs(24 * 60 * 60);

pub async fn remove_orphaned_vm_dirs(
    root: &Path,
    desired_ids: &HashSet<&str>,
    now: SystemTime,
) -> usize {
    if desired_ids.is_empty() {
        return 0;
    }
    let mut removed = 0;
    for path in entries(root).await {
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if desired_ids.contains(name) || !is_expired(&path, now).await {
            continue;
        }
        match tokio::fs::remove_dir_all(&path).await {
            Ok(()) => {
                info!(path = ?path, "Removed orphaned vm directory");
                removed += 1;
            }
            Err(e) => warn!(path = ?path, error = %e, "Failed to remove orphaned vm directory"),
        }
    }
    removed
}

pub async fn remove_superseded_rootfs(cache_dir: &Path, now: SystemTime) -> usize {
    let mut newest: HashMap<String, SystemTime> = HashMap::new();
    let mut images = Vec::new();
    for path in entries(cache_dir).await {
        let (Some(key), Some(modified)) = (image_key(&path), modified(&path).await) else {
            continue;
        };
        let latest = newest.entry(key.clone()).or_insert(modified);
        *latest = (*latest).max(modified);
        images.push((path, key, modified));
    }

    let mut removed = 0;
    for (path, key, modified) in images {
        if modified >= newest[&key] || !older_than_grace(modified, now) {
            continue;
        }
        match tokio::fs::remove_file(&path).await {
            Ok(()) => {
                info!(path = ?path, "Removed superseded rootfs image");
                removed += 1;
            }
            Err(e) => warn!(path = ?path, error = %e, "Failed to remove superseded rootfs image"),
        }
    }
    removed
}

fn image_key(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    name.strip_suffix(".ext4")?;
    let mut parts = name.splitn(3, '-');
    Some(format!("{}-{}", parts.next()?, parts.next()?))
}

async fn entries(dir: &Path) -> Vec<PathBuf> {
    let Ok(mut reader) = tokio::fs::read_dir(dir).await else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    while let Ok(Some(entry)) = reader.next_entry().await {
        paths.push(entry.path());
    }
    paths
}

async fn modified(path: &Path) -> Option<SystemTime> {
    tokio::fs::metadata(path).await.ok()?.modified().ok()
}

async fn is_expired(path: &Path, now: SystemTime) -> bool {
    modified(path)
        .await
        .is_some_and(|modified| older_than_grace(modified, now))
}

fn older_than_grace(modified: SystemTime, now: SystemTime) -> bool {
    now.duration_since(modified)
        .is_ok_and(|age| age >= GRACE_PERIOD)
}

#[cfg(test)]
mod tests;
