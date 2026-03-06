use crate::{log_warn};
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::ceph::ops::CephManager;
use crate::db::volumes as db;
use crate::models::volume::{
    AttachVolumeRequest, CreateSnapshotRequest, CreateVolumeRequest, SnapshotResponse,
    VolumeResponse,
};

pub struct VolumeService {
    db: DatabaseConnection,
    _etcd: Arc<Mutex<EtcdClient>>,
    ceph: Option<Arc<CephManager>>,
}

impl VolumeService {
    pub fn new(
        db: DatabaseConnection,
        etcd: Arc<Mutex<EtcdClient>>,
        ceph: Option<Arc<CephManager>>,
    ) -> Self {
        Self { db, _etcd: etcd, ceph }
    }

    pub async fn create_volume(&self, req: CreateVolumeRequest) -> Result<VolumeResponse, String> {
        let model = db::create(&self.db, &req)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(ceph) = &self.ceph {
            let volume = crate::ceph::storage::types::CephVolume {
                name: model.image_name.clone(),
                pool: model.pool.clone(),
                size_mb: (model.size_gb as u64) * 1024,
                features: vec!["layering".to_string(), "exclusive-lock".to_string()],
                encrypted: false,
            };

            if let Err(e) = ceph.rbd_manager.create_image(&volume).await {
                log_warn!("volume_service", &format!("Ceph RBD create failed: {}", e));
            }
        }

        Ok(db::into_response(model))
    }

    pub async fn list_volumes(&self) -> Result<Vec<VolumeResponse>, String> {
        db::get_all(&self.db).await.map_err(|e| e.to_string())
    }

    pub async fn get_volume(&self, id: Uuid) -> Result<Option<VolumeResponse>, String> {
        let model = db::get_by_id(&self.db, id)
            .await
            .map_err(|e| e.to_string())?;
        Ok(model.map(db::into_response))
    }

    pub async fn delete_volume(&self, id: Uuid) -> Result<(), String> {
        let model = db::get_by_id(&self.db, id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Volume {} not found", id))?;

        if let Some(ceph) = &self.ceph {
            if let Err(e) = ceph.rbd_manager.delete_image(&model.pool, &model.image_name).await {
                log_warn!("volume_service", &format!("Ceph RBD delete failed: {}", e));
            }
        }

        db::delete(&self.db, id).await.map_err(|e| e.to_string())
    }

    pub async fn attach_volume(
        &self,
        volume_id: Uuid,
        req: AttachVolumeRequest,
    ) -> Result<VolumeResponse, String> {
        let model = db::get_by_id(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Volume {} not found", volume_id))?;

        let device = if let Some(ceph) = &self.ceph {
            match ceph.rbd_manager.map_device(&model.pool, &model.image_name).await {
                Ok(dev) => Some(dev),
                Err(e) => {
                    log_warn!("volume_service", &format!("Ceph RBD map failed: {}", e));
                    None
                }
            }
        } else {
            None
        };

        let updated = db::attach(&self.db, volume_id, req.agent_id, req.workload_id, device)
            .await
            .map_err(|e| e.to_string())?;

        Ok(db::into_response(updated))
    }

    pub async fn detach_volume(&self, volume_id: Uuid) -> Result<VolumeResponse, String> {
        let model = db::get_by_id(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Volume {} not found", volume_id))?;

        if let Some(ceph) = &self.ceph {
            if let Some(ref device) = model.mapped_device {
                if let Err(e) = ceph.rbd_manager.unmap_device(device).await {
                    log_warn!("volume_service", &format!("Ceph RBD unmap failed: {}", e));
                }
            }
        }

        let updated = db::detach(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())?;

        Ok(db::into_response(updated))
    }

    pub async fn create_snapshot(
        &self,
        volume_id: Uuid,
        req: CreateSnapshotRequest,
    ) -> Result<SnapshotResponse, String> {
        let model = db::get_by_id(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Volume {} not found", volume_id))?;

        if let Some(ceph) = &self.ceph {
            if let Err(e) = ceph
                .rbd_manager
                .create_snapshot(&model.pool, &model.image_name, &req.name)
                .await
            {
                log_warn!("volume_service", &format!("Ceph snapshot create failed: {}", e));
            }
        }

        let snap = db::create_snapshot(&self.db, volume_id, &req.name)
            .await
            .map_err(|e| e.to_string())?;

        Ok(SnapshotResponse {
            id: snap.id,
            volume_id: snap.volume_id,
            name: snap.name,
            status: crate::models::volume::SnapshotStatus::Available,
            created_at: snap.created_at.and_utc(),
        })
    }

    pub async fn list_snapshots(&self, volume_id: Uuid) -> Result<Vec<SnapshotResponse>, String> {
        db::list_snapshots(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())
    }
}
