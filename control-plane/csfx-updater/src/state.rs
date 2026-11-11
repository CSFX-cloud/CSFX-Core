use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::manifest::Manifest;

const APPLIED_FILE: &str = "applied.json";
const PENDING_FILE: &str = "pending.json";

#[derive(Debug, Serialize, Deserialize)]
pub struct Pending {
    pub previous_toplevel: String,
    pub manifest: Manifest,
}

fn read<T: DeserializeOwned>(path: PathBuf) -> Result<Option<T>> {
    match std::fs::read(&path) {
        Ok(bytes) => {
            Ok(Some(serde_json::from_slice(&bytes).with_context(|| {
                format!("corrupt state file {}", path.display())
            })?))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
    }
}

fn write<T: Serialize>(path: PathBuf, value: &T) -> Result<()> {
    let staging = path.with_extension("tmp");
    std::fs::create_dir_all(path.parent().context("state path has no parent")?)?;
    std::fs::write(&staging, serde_json::to_vec(value)?)?;
    std::fs::rename(&staging, &path)?;
    Ok(())
}

pub fn load_applied(dir: &Path) -> Result<Option<Manifest>> {
    read(dir.join(APPLIED_FILE))
}

pub fn save_applied(dir: &Path, manifest: &Manifest) -> Result<()> {
    write(dir.join(APPLIED_FILE), manifest)
}

pub fn load_pending(dir: &Path) -> Result<Option<Pending>> {
    read(dir.join(PENDING_FILE))
}

pub fn save_pending(dir: &Path, pending: &Pending) -> Result<()> {
    write(dir.join(PENDING_FILE), pending)
}

pub fn clear_pending(dir: &Path) -> Result<()> {
    match std::fs::remove_file(dir.join(PENDING_FILE)) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        Manifest {
            version: "0.3.0".into(),
            arch: "x86_64-linux".into(),
            toplevel: "/nix/store/abc-system".into(),
            bundle: "b.tar.zst".into(),
            released_at: "now".into(),
        }
    }

    #[test]
    fn pending_state_round_trips_and_clears() {
        let dir = std::env::temp_dir().join(format!("csfx-updater-state-{}", std::process::id()));
        assert!(load_pending(&dir).unwrap().is_none());
        let pending = Pending {
            previous_toplevel: "/nix/store/old".into(),
            manifest: manifest(),
        };
        save_pending(&dir, &pending).unwrap();
        assert_eq!(load_pending(&dir).unwrap().unwrap().manifest, manifest());
        clear_pending(&dir).unwrap();
        assert!(load_pending(&dir).unwrap().is_none());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
