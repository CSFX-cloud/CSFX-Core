use axum::body::Bytes;
use futures_util::{stream, Stream, StreamExt};
use std::{fmt::Display, io, pin::Pin, sync::Arc};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    process::ChildStdout,
};
use uuid::Uuid;

use super::volume::{ServiceError, VolumeService, IMAGE_FEATURES};
use crate::{
    ceph::{core::CephProcess, ops::CephManager},
    db::volumes as db,
    log_error,
    models::volume::{CreateVolumeRequest, VolumeResponse, VolumeStatus},
};

const READ_CHUNK: usize = 1 << 20;
const SNAPSHOT_PREFIX: &str = "csfx-export-";

pub type ExportStream = Pin<Box<dyn Stream<Item = io::Result<Bytes>> + Send>>;

struct SnapshotGuard {
    ceph: Arc<CephManager>,
    pool: String,
    image: String,
    snapshot: String,
}

impl Drop for SnapshotGuard {
    fn drop(&mut self) {
        let ceph = self.ceph.clone();
        let (pool, image, snapshot) = (
            std::mem::take(&mut self.pool),
            std::mem::take(&mut self.image),
            std::mem::take(&mut self.snapshot),
        );
        tokio::spawn(async move {
            if let Err(error) = ceph
                .rbd_manager
                .delete_snapshot(&pool, &image, &snapshot)
                .await
            {
                log_error!(
                    "transfer",
                    &format!("export snapshot cleanup failed {pool}/{image}@{snapshot}: {error:#}")
                );
            }
        });
    }
}

struct Exporting {
    stdout: ChildStdout,
    process: Option<CephProcess>,
    _guard: SnapshotGuard,
}

async fn next_chunk(mut state: Exporting) -> Option<(io::Result<Bytes>, Exporting)> {
    let mut buffer = vec![0u8; READ_CHUNK];
    match state.stdout.read(&mut buffer).await {
        Ok(0) => {
            let process = state.process.take()?;
            match process.finish().await {
                Ok(()) => None,
                Err(error) => Some((Err(io::Error::other(format!("{error:#}"))), state)),
            }
        }
        Ok(read) => {
            buffer.truncate(read);
            Some((Ok(Bytes::from(buffer)), state))
        }
        Err(error) => Some((Err(error), state)),
    }
}

impl VolumeService {
    fn ceph_manager(&self) -> Result<Arc<CephManager>, ServiceError> {
        self.ceph
            .clone()
            .ok_or_else(|| ServiceError::Internal("ceph is not configured".to_string()))
    }

    pub async fn export_volume(&self, id: Uuid) -> Result<Option<ExportStream>, ServiceError> {
        let Some(model) = db::get_by_id(&self.db, id)
            .await
            .map_err(|e| e.to_string())?
        else {
            return Ok(None);
        };
        let status = VolumeStatus::from_str(&model.status);
        if matches!(status, VolumeStatus::Creating | VolumeStatus::Deleting) {
            return Err(ServiceError::Conflict(format!(
                "Volume {id} is {} and cannot be exported",
                model.status
            )));
        }
        let ceph = self.ceph_manager()?;
        let snapshot = format!("{SNAPSHOT_PREFIX}{}", Uuid::new_v4());
        ceph.rbd_manager
            .create_snapshot(&model.pool, &model.image_name, &snapshot)
            .await
            .map_err(|e| format!("{e:#}"))?;
        let guard = SnapshotGuard {
            ceph: ceph.clone(),
            pool: model.pool.clone(),
            image: model.image_name.clone(),
            snapshot: snapshot.clone(),
        };
        let mut process = ceph
            .rbd_manager
            .export_stream(&model.pool, &model.image_name, &snapshot)
            .await
            .map_err(|e| format!("{e:#}"))?;
        let stdout = process
            .child
            .stdout
            .take()
            .ok_or_else(|| ServiceError::Internal("rbd export has no stdout".to_string()))?;
        let state = Exporting {
            stdout,
            process: Some(process),
            _guard: guard,
        };
        Ok(Some(Box::pin(stream::unfold(state, next_chunk))))
    }

    async fn write_image<S, E>(
        &self,
        ceph: &CephManager,
        pool: &str,
        image: &str,
        mut source: S,
    ) -> Result<(), String>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: Display,
    {
        let mut process = ceph
            .rbd_manager
            .import_stream(pool, image, &IMAGE_FEATURES)
            .await
            .map_err(|e| format!("{e:#}"))?;
        let mut stdin = process
            .child
            .stdin
            .take()
            .ok_or_else(|| "rbd import has no stdin".to_string())?;
        while let Some(chunk) = source.next().await {
            let chunk = chunk.map_err(|e| e.to_string())?;
            stdin.write_all(&chunk).await.map_err(|e| e.to_string())?;
        }
        drop(stdin);
        process.finish().await.map_err(|e| format!("{e:#}"))
    }

    pub async fn import_volume<S, E>(
        &self,
        request: CreateVolumeRequest,
        source: S,
    ) -> Result<VolumeResponse, ServiceError>
    where
        S: Stream<Item = Result<Bytes, E>> + Unpin,
        E: Display,
    {
        let ceph = self.ceph_manager()?;
        let model = db::create(&self.db, &request)
            .await
            .map_err(|e| e.to_string())?;
        match self
            .write_image(&ceph, &model.pool, &model.image_name, source)
            .await
        {
            Ok(()) => Ok(self.mark(model.id, VolumeStatus::Available).await?),
            Err(error) => {
                if let Err(cleanup) = self.finish_delete(&model).await {
                    log_error!(
                        "transfer",
                        &format!("import cleanup failed volume={}: {cleanup}", model.id)
                    );
                }
                Err(ServiceError::Internal(error))
            }
        }
    }
}
