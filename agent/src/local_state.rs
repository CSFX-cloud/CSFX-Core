use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;
use tracing::{info, warn};

use crate::client::{AssignedVolume, AssignedWorkload};

pub const STATE_FILE: &str = "/var/lib/csfx-agent/desired-state.json";

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
struct Snapshot {
    generation: u64,
    workloads: Option<Vec<AssignedWorkload>>,
    volumes: Option<Vec<AssignedVolume>>,
}

pub struct LocalState {
    path: PathBuf,
    snapshot: Mutex<Snapshot>,
}

impl LocalState {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let snapshot = read_snapshot(&path);
        Self {
            path,
            snapshot: Mutex::new(snapshot),
        }
    }

    pub async fn confirm_workloads(&self, workloads: &[AssignedWorkload]) {
        let mut normalized = workloads.to_vec();
        for workload in &mut normalized {
            workload.restart_requested = false;
        }
        self.update(|snapshot| replace(&mut snapshot.workloads, normalized))
            .await;
    }

    pub async fn confirm_volumes(&self, volumes: &[AssignedVolume]) {
        self.update(|snapshot| replace(&mut snapshot.volumes, volumes.to_vec()))
            .await;
    }

    pub async fn workloads(&self) -> Option<Vec<AssignedWorkload>> {
        self.snapshot.lock().await.workloads.clone()
    }

    pub async fn volumes(&self) -> Option<Vec<AssignedVolume>> {
        self.snapshot.lock().await.volumes.clone()
    }

    pub async fn generation(&self) -> u64 {
        self.snapshot.lock().await.generation
    }

    pub async fn clear(&self) {
        let mut snapshot = self.snapshot.lock().await;
        let generation = snapshot.generation + 1;
        *snapshot = Snapshot {
            generation,
            ..Snapshot::default()
        };
        if let Err(e) = tokio::fs::remove_file(&self.path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                warn!(path = %self.path.display(), error = %e, "Failed to remove local workload state");
            }
        }
    }

    async fn update(&self, change: impl FnOnce(&mut Snapshot) -> bool) {
        let mut snapshot = self.snapshot.lock().await;
        let mut next = snapshot.clone();
        if !change(&mut next) {
            return;
        }
        next.generation += 1;
        if let Err(e) = persist(&self.path, &next).await {
            warn!(path = %self.path.display(), error = %e, "Failed to persist local workload state");
        }
        *snapshot = next;
    }
}

fn replace<T: PartialEq>(slot: &mut Option<T>, value: T) -> bool {
    if slot.as_ref() == Some(&value) {
        return false;
    }
    *slot = Some(value);
    true
}

fn read_snapshot(path: &Path) -> Snapshot {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Snapshot::default(),
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Failed to read local workload state");
            return Snapshot::default();
        }
    };
    match serde_json::from_slice::<Snapshot>(&raw) {
        Ok(snapshot) => {
            info!(
                generation = snapshot.generation,
                "Local workload state loaded"
            );
            snapshot
        }
        Err(e) => {
            warn!(path = %path.display(), error = %e, "Ignoring unreadable local workload state");
            Snapshot::default()
        }
    }
}

async fn persist(path: &Path, snapshot: &Snapshot) -> Result<()> {
    let body = serde_json::to_vec(snapshot).context("Failed to serialize local workload state")?;
    let staging = path.with_extension("tmp");
    let mut file = tokio::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&staging)
        .await
        .context("Failed to open staging file")?;
    file.write_all(&body)
        .await
        .context("Failed to write staging file")?;
    file.sync_all()
        .await
        .context("Failed to sync staging file")?;
    tokio::fs::rename(&staging, path)
        .await
        .context("Failed to move staging file into place")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_path() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("csfx-local-state-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("desired-state.json")
    }

    fn workload(id: &str, restart_requested: bool) -> AssignedWorkload {
        serde_json::from_value(serde_json::json!({
            "id": id, "image": "img", "cpu_millicores": 100, "memory_bytes": 1,
            "env_vars": null, "ports": null, "volume_mounts": null, "service_name": null,
            "restart_policy": "always", "max_restarts": null, "resource_group_id": null,
            "resource_group_cidr": null, "restart_requested": restart_requested,
            "runtime_class": "microvm", "disk_bytes": 0
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn confirmed_workloads_survive_a_restart() {
        let path = state_path();
        LocalState::open(&path)
            .confirm_workloads(&[workload("w1", false)])
            .await;

        let reopened = LocalState::open(&path);

        let workloads = reopened.workloads().await.unwrap();
        assert_eq!(workloads.len(), 1);
        assert_eq!(workloads[0].id, "w1");
        assert_eq!(reopened.generation().await, 1);
    }

    #[tokio::test]
    async fn the_generation_only_advances_on_change() {
        let state = LocalState::open(state_path());
        state.confirm_workloads(&[workload("w1", false)]).await;
        state.confirm_workloads(&[workload("w1", false)]).await;
        assert_eq!(state.generation().await, 1);
        state.confirm_workloads(&[workload("w2", false)]).await;
        assert_eq!(state.generation().await, 2);
    }

    #[tokio::test]
    async fn restart_requests_are_never_replayed() {
        let state = LocalState::open(state_path());
        state.confirm_workloads(&[workload("w1", true)]).await;
        assert!(!state.workloads().await.unwrap()[0].restart_requested);
    }

    #[tokio::test]
    async fn clearing_forgets_everything_on_disk_too() {
        let path = state_path();
        let state = LocalState::open(&path);
        state.confirm_workloads(&[workload("w1", false)]).await;
        state.clear().await;
        assert!(state.workloads().await.is_none());
        assert!(LocalState::open(&path).workloads().await.is_none());
    }

    #[tokio::test]
    async fn unreadable_files_are_ignored() {
        let path = state_path();
        std::fs::write(&path, b"not json").unwrap();
        assert!(LocalState::open(&path).workloads().await.is_none());
    }

    #[tokio::test]
    async fn the_file_is_private() {
        use std::os::unix::fs::PermissionsExt;
        let path = state_path();
        LocalState::open(&path)
            .confirm_workloads(&[workload("w1", false)])
            .await;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}
