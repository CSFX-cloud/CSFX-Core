use crate::error::ServiceError;
use chrono::Utc;
use entity::{site_artifacts, SiteArtifacts};
use flate2::read::GzDecoder;
use sea_orm::{sea_query::OnConflict, DatabaseConnection, EntityTrait, PaginatorTrait, Set};
use std::collections::HashMap;
use std::io::Read;
use uuid::Uuid;

pub const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;
const MAX_UNPACKED_BYTES: u64 = 256 * 1024 * 1024;

pub struct Archive {
    pub files: HashMap<String, Vec<u8>>,
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
        let mut data = Vec::new();
        entry
            .take(MAX_UNPACKED_BYTES.saturating_sub(total) + 1)
            .read_to_end(&mut data)
            .map_err(invalid)?;
        total += data.len() as u64;
        if total > MAX_UNPACKED_BYTES {
            return Err(ServiceError::Archive("archive is too large".to_string()));
        }
        files.insert(name, data);
    }
    Ok(Archive { files })
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
        assert_eq!(archive.files["index.html"], b"hi");
        assert_eq!(archive.files["assets/app.js"], b"js");
    }

    #[test]
    fn unpack_rejects_garbage() {
        assert!(unpack(b"not an archive").is_err());
    }
}
