use crate::encoding::{self, Encoding};
use crate::error::ServiceError;
use axum::body::Bytes;
use chrono::Utc;
use entity::{site_artifacts, SiteArtifacts};
use flate2::read::GzDecoder;
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait, PaginatorTrait, Set};
use std::collections::HashMap;
use std::io::Read;
use std::sync::Mutex;
use uuid::Uuid;

pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone)]
pub struct Variant {
    pub data: Bytes,
    pub etag: String,
    pub encoding: Encoding,
}

pub struct Archive {
    pub files: HashMap<String, Bytes>,
    variants: Mutex<HashMap<(String, Encoding), Variant>>,
}

impl Archive {
    pub fn new(files: HashMap<String, Bytes>) -> Self {
        Self {
            files,
            variants: Mutex::new(HashMap::new()),
        }
    }

    pub fn size(&self) -> usize {
        self.files.values().map(Bytes::len).sum()
    }

    pub fn variant(&self, path: &str, encoding: Encoding) -> Option<Variant> {
        let key = (path.to_string(), encoding);
        if let Some(found) = self.remembered(&key) {
            return Some(found);
        }
        let built = self.build_variant(path, encoding)?;
        self.variants
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(key, built.clone());
        Some(built)
    }

    fn remembered(&self, key: &(String, Encoding)) -> Option<Variant> {
        self.variants
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(key)
            .cloned()
    }

    fn build_variant(&self, path: &str, encoding: Encoding) -> Option<Variant> {
        let original = self.files.get(path)?;
        let hash = encoding::content_hash(original);
        let Some((data, token)) = encoding::compress(original, encoding).zip(encoding.token())
        else {
            return Some(Variant {
                data: original.clone(),
                etag: format!("\"{hash}\""),
                encoding: Encoding::Identity,
            });
        };
        Some(Variant {
            data,
            etag: format!("\"{hash}-{token}\""),
            encoding,
        })
    }
}

pub async fn exists(db: &DatabaseConnection, id: Uuid) -> Result<bool, ServiceError> {
    Ok(SiteArtifacts::find_by_id(id).count(db).await? > 0)
}

pub async fn store(
    db: &DatabaseConnection,
    id: Uuid,
    content: Vec<u8>,
) -> Result<(), ServiceError> {
    unpack(&content)?;
    let size = i64::try_from(content.len()).unwrap_or(i64::MAX);
    let row = site_artifacts::ActiveModel {
        deployment_id: Set(id),
        content: Set(content),
        size_bytes: Set(size),
        created_at: Set(Utc::now().naive_utc()),
    };
    SiteArtifacts::insert(row)
        .on_conflict(
            OnConflict::column(site_artifacts::Column::DeploymentId)
                .update_columns([
                    site_artifacts::Column::Content,
                    site_artifacts::Column::SizeBytes,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(())
}

pub async fn load(db: &DatabaseConnection, id: Uuid) -> Result<Option<Archive>, ServiceError> {
    match SiteArtifacts::find_by_id(id).one(db).await? {
        Some(row) => unpack(&row.content).map(Some),
        None => Ok(None),
    }
}

fn clean_path(raw: &std::path::Path) -> Option<String> {
    let mut parts = Vec::new();
    for component in raw.components() {
        match component {
            std::path::Component::Normal(part) => parts.push(part.to_str()?),
            std::path::Component::CurDir => {}
            _ => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

fn is_repository_metadata(name: &str) -> bool {
    name.split('/').any(|part| part == ".git")
}

pub fn unpack(content: &[u8]) -> Result<Archive, ServiceError> {
    let invalid = |error: std::io::Error| ServiceError::Archive(error.to_string());
    let mut archive = tar::Archive::new(GzDecoder::new(content));
    let mut files = HashMap::new();
    let mut total: u64 = 0;
    for entry in archive.entries().map_err(invalid)? {
        let entry = entry.map_err(invalid)?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path().map_err(invalid)?.into_owned();
        let Some(name) = clean_path(&path) else {
            return Err(ServiceError::Archive("unsafe path in archive".to_string()));
        };
        if is_repository_metadata(&name) {
            continue;
        }
        let mut data = Vec::new();
        entry
            .take(MAX_UNPACKED_BYTES.saturating_sub(total) + 1)
            .read_to_end(&mut data)
            .map_err(invalid)?;
        total += data.len() as u64;
        if total > MAX_UNPACKED_BYTES {
            return Err(ServiceError::Archive("archive is too large".to_string()));
        }
        files.insert(name, Bytes::from(data));
    }
    Ok(Archive::new(files))
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{write::GzEncoder, Compression};

    fn archive_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        for (name, data) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_size(data.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            builder.append_data(&mut header, name, *data).unwrap();
        }
        let tar_bytes = builder.into_inner().unwrap();
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        std::io::Write::write_all(&mut encoder, &tar_bytes).unwrap();
        encoder.finish().unwrap()
    }

    #[test]
    fn unpack_reads_regular_files_and_normalizes_paths() {
        let bytes = archive_with(&[("./index.html", b"hi"), ("assets/app.js", b"js")]);
        let archive = unpack(&bytes).unwrap();
        assert_eq!(&archive.files["index.html"][..], b"hi");
        assert_eq!(&archive.files["assets/app.js"][..], b"js");
    }

    #[test]
    fn unpack_drops_repository_metadata() {
        let bytes = archive_with(&[
            ("index.html", b"hi"),
            (".git/config", b"[core]"),
            ("docs/.git/HEAD", b"ref"),
            (".github/workflow.yml", b"ci"),
            (".gitignore", b"x"),
        ]);
        let archive = unpack(&bytes).unwrap();
        let mut names: Vec<&str> = archive.files.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(
            names,
            vec![".github/workflow.yml", ".gitignore", "index.html"]
        );
    }

    #[test]
    fn unpack_rejects_garbage() {
        assert!(unpack(b"not an archive").is_err());
    }
}
