use crate::log_info;
use entity::entities::volumes;
use etcd_client::Client as EtcdClient;
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::ceph::ops::CephManager;
use crate::ceph::storage::types::CephVolume;
use crate::db::volumes as db;
use crate::models::volume::{
    AttachVolumeRequest, CreateSnapshotRequest, CreateVolumeRequest, SnapshotResponse,
    VolumeResponse,
};

const IMAGE_FEATURES: [&str; 2] = ["layering", "exclusive-lock"];

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
        Self {
            db,
            _etcd: etcd,
            ceph,
        }
    }

    pub async fn create_volume(&self, req: CreateVolumeRequest) -> Result<VolumeResponse, String> {
        let model = db::create(&self.db, &req)
            .await
            .map_err(|e| e.to_string())?;

        if let Some(ceph) = &self.ceph {
            let volume = CephVolume {
                name: model.image_name.clone(),
                pool: model.pool.clone(),
                size_mb: (model.size_gb as u64) * 1024,
                features: IMAGE_FEATURES.iter().map(|f| f.to_string()).collect(),
            };

            if let Err(e) = ceph.rbd_manager.create_image(&volume).await {
                db::delete(&self.db, model.id)
                    .await
                    .map_err(|rollback| format!("{:#}; rollback failed: {}", e, rollback))?;
                return Err(format!("{:#}", e));
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
            let rbd = &ceph.rbd_manager;
            let exists = rbd
                .image_exists(&model.pool, &model.image_name)
                .await
                .map_err(|e| format!("{:#}", e))?;
            if exists {
                rbd.delete_image(&model.pool, &model.image_name)
                    .await
                    .map_err(|e| format!("{:#}", e))?;
            }
        }

        db::delete(&self.db, id).await.map_err(|e| e.to_string())
    }

    pub async fn attach_volume(
        &self,
        volume_id: Uuid,
        req: AttachVolumeRequest,
    ) -> Result<VolumeResponse, String> {
        db::get_by_id(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Volume {} not found", volume_id))?;

        let updated = db::attach(&self.db, volume_id, req.agent_id, req.workload_id)
            .await
            .map_err(|e| e.to_string())?;

        Ok(db::into_response(updated))
    }

    pub async fn detach_volume(&self, volume_id: Uuid) -> Result<VolumeResponse, String> {
        db::get_by_id(&self.db, volume_id)
            .await
            .map_err(|e| e.to_string())?
            .ok_or_else(|| format!("Volume {} not found", volume_id))?;

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
            ceph.rbd_manager
                .create_snapshot(&model.pool, &model.image_name, &req.name)
                .await
                .map_err(|e| format!("{:#}", e))?;
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

    pub async fn force_detach_all(&self, agent_id: Uuid) -> Result<Vec<Uuid>, String> {
        let attached = db::get_attached_to_agent(&self.db, agent_id)
            .await
            .map_err(|e| e.to_string())?;

        let mut detached_ids = Vec::with_capacity(attached.len());

        for volume in attached {
            self.fence_volume(&volume).await?;

            db::detach(&self.db, volume.id)
                .await
                .map_err(|e| e.to_string())?;

            detached_ids.push(volume.id);
        }

        Ok(detached_ids)
    }

    async fn fence_volume(&self, volume: &volumes::Model) -> Result<(), String> {
        let Some(ceph) = &self.ceph else {
            return Ok(());
        };

        let addresses = ceph
            .rbd_manager
            .watcher_addresses(&volume.pool, &volume.image_name)
            .await
            .map_err(|e| format!("{:#}", e))?;

        for address in addresses {
            ceph.client
                .blocklist_add(&address)
                .await
                .map_err(|e| format!("{:#}", e))?;
            log_info!(
                "volume_service",
                &format!(
                    "Fenced RBD client volume_id={} address={}",
                    volume.id, address
                )
            );
        }

        Ok(())
    }
}
