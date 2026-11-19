use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{info, warn};

use super::Reconciler;
use crate::{client, rbd};

pub type MountedVolumes = Arc<Mutex<HashMap<String, String>>>;

impl Reconciler {
    pub(super) async fn process_volumes(&self) -> Option<Vec<client::AssignedVolume>> {
        let volumes = match self
            .client
            .fetch_assigned_volumes()
            .await
        {
            Ok(volumes) => volumes,
            Err(e) => {
                warn!(error = %e, "Failed to fetch assigned volumes");
                if let Some(cached) = self.local_state.volumes().await {
                    map_missing_volumes(&cached, &self.volumes).await;
                }
                return None;
            }
        };

        self.local_state.confirm_volumes(&volumes).await;
        map_missing_volumes(&volumes, &self.volumes).await;
        grow_mapped_volumes(&volumes, &self.volumes).await;
        Some(volumes)
    }

    pub(super) async fn release_unassigned_volumes(&self, assigned: &[client::AssignedVolume]) {
        let assigned_ids: HashSet<&str> = assigned.iter().map(|v| v.id.as_str()).collect();

        let stale: Vec<(String, String)> = self
            .volumes
            .lock()
            .await
            .iter()
            .filter(|(volume_id, _)| !assigned_ids.contains(volume_id.as_str()))
            .map(|(volume_id, device)| (volume_id.clone(), device.clone()))
            .collect();

        for (volume_id, device) in stale {
            if let Err(e) = rbd::unmap_device(&device).await {
                warn!(volume_id = %volume_id, device = %device, error = %e, "Failed to unmap RBD device");
                continue;
            }
            self.volumes.lock().await.remove(&volume_id);
            info!(volume_id = %volume_id, "Volume released");
        }
    }

    pub(super) async fn volumes_ready(&self, workload: &client::AssignedWorkload) -> bool {
        let Some(mounts) = &workload.volume_mounts else {
            return true;
        };
        let mounted = self.volumes.lock().await;
        mounts
            .iter()
            .all(|mount| mounted.contains_key(&mount.volume_id))
    }
}

async fn grow_mapped_volumes(volumes: &[client::AssignedVolume], mounted: &MountedVolumes) {
    for volume in volumes.iter().filter(|volume| volume.size_gb > 0) {
        let Some(device) = mounted.lock().await.get(&volume.id).cloned() else {
            continue;
        };
        if let Err(e) = rbd::grow_image_to(
            &volume.pool,
            &volume.image_name,
            &device,
            volume.size_gb as u64,
        )
        .await
        {
            warn!(volume_id = %volume.id, device = %device, error = %e, "Failed to grow volume image");
        }
    }
}

async fn map_missing_volumes(volumes: &[client::AssignedVolume], mounted: &MountedVolumes) {
    let missing: Vec<&client::AssignedVolume> = {
        let mounted = mounted.lock().await;
        volumes
            .iter()
            .filter(|volume| !mounted.contains_key(&volume.id))
            .collect()
    };
    if missing.is_empty() {
        return;
    }

    let existing = match rbd::list_mapped().await {
        Ok(mapped) => mapped,
        Err(e) => {
            warn!(error = %e, "Failed to list mapped RBD devices");
            return;
        }
    };

    for volume in missing {
        attach_volume_device(volume, &existing, mounted).await;
    }
}

async fn attach_volume_device(
    volume: &client::AssignedVolume,
    existing: &[rbd::MappedImage],
    mounted: &MountedVolumes,
) {
    let adopted = existing
        .iter()
        .find(|mapped| mapped.pool == volume.pool && mapped.image == volume.image_name);

    let device = match adopted {
        Some(mapped) => mapped.device.clone(),
        None => match rbd::map_device(&volume.pool, &volume.image_name).await {
            Ok(device) => device,
            Err(e) => {
                warn!(volume_id = %volume.id, error = %e, "Failed to map RBD device");
                return;
            }
        },
    };

    let label = format!("csfx-{}", volume.id.chars().take(11).collect::<String>());
    if let Err(e) = rbd::ensure_filesystem(&device, &label).await {
        warn!(volume_id = %volume.id, device = %device, error = %e, "Failed to prepare volume filesystem");
        return;
    }

    mounted
        .lock()
        .await
        .insert(volume.id.clone(), device.clone());

    info!(volume_id = %volume.id, device = %device, adopted = adopted.is_some(), "Volume mapped");
}

#[cfg(test)]
mod tests {
    use super::super::fixtures::{json_client, local_state, reconciler};

    #[tokio::test]
    async fn volumes_that_cannot_be_mapped_stay_unmounted() {
        let client = json_client(&[(
            "/api/agents/self/volumes",
            serde_json::json!([{
                "id": "v1", "pool": "p", "image_name": "i", "status": "in_use"
            }]),
        )])
        .await;
        let reconciler = reconciler(client, local_state());
        reconciler.process_volumes().await;
        assert!(reconciler.volumes.lock().await.is_empty());

        reconciler
            .volumes
            .lock()
            .await
            .insert("old".to_string(), "/dev/missing".to_string());
        reconciler.release_unassigned_volumes(&[]).await;
        assert!(reconciler.volumes.lock().await.contains_key("old"));
    }
}
