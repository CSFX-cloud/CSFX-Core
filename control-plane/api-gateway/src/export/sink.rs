use axum::body::Bytes;
use futures_util::{stream, StreamExt};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use tokio::{
    fs,
    io::{AsyncReadExt, AsyncWriteExt},
};
use uuid::Uuid;

use super::{
    s3::{S3Config, S3Sink},
    ByteStream, ExportError,
};

const READ_CHUNK: usize = 1 << 20;
const PART_SIZE: usize = 64 << 20;

#[derive(Debug, Error)]
pub enum SinkError {
    #[error("invalid artifact name: {0}")]
    InvalidName(String),
    #[error("target io failed: {0}")]
    Io(#[from] std::io::Error),
    #[error("target request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("{0}")]
    Remote(String),
}

#[derive(Clone)]
pub enum Sink {
    Local(PathBuf),
    S3(S3Sink),
}

pub enum Writer {
    Local {
        file: fs::File,
        temporary: PathBuf,
        path: PathBuf,
    },
    S3(S3Writer),
}

pub struct S3Writer {
    sink: S3Sink,
    key: String,
    buffer: Vec<u8>,
    upload_id: Option<String>,
    etags: Vec<String>,
}

fn validated(name: &str) -> Result<&Path, SinkError> {
    let path = Path::new(name);
    let clean = !name.is_empty()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)));
    if clean {
        Ok(path)
    } else {
        Err(SinkError::InvalidName(name.to_string()))
    }
}

fn object_name(export_id: Uuid, name: &str) -> Result<String, SinkError> {
    validated(name)?;
    Ok(format!("{export_id}/{name}"))
}

impl Sink {
    pub fn s3(config: S3Config) -> Self {
        Self::S3(S3Sink::new(config))
    }

    pub async fn writer(&self, export_id: Uuid, name: &str) -> Result<Writer, SinkError> {
        let object = object_name(export_id, name)?;
        match self {
            Self::Local(root) => open_local(root.join(&object)).await,
            Self::S3(sink) => Ok(Writer::S3(S3Writer {
                sink: sink.clone(),
                key: object,
                buffer: Vec::new(),
                upload_id: None,
                etags: Vec::new(),
            })),
        }
    }

    pub async fn write(&self, export_id: Uuid, name: &str, bytes: Bytes) -> Result<(), SinkError> {
        let mut writer = self.writer(export_id, name).await?;
        if let Err(error) = writer.write(bytes).await {
            writer.abort().await;
            return Err(error);
        }
        writer.finish().await
    }

    pub async fn open(&self, export_id: Uuid, name: &str) -> Result<ByteStream, SinkError> {
        let object = object_name(export_id, name)?;
        match self {
            Self::Local(root) => open_local_stream(root.join(object)).await,
            Self::S3(sink) => {
                let response = sink.get(&object).await?;
                Ok(Box::pin(response.bytes_stream().map(|chunk| {
                    chunk.map_err(|error| ExportError::Sink(error.into()))
                })))
            }
        }
    }

    pub async fn remove(&self, export_id: Uuid, name: &str) -> Result<(), SinkError> {
        let object = object_name(export_id, name)?;
        match self {
            Self::Local(root) => {
                let path = root.join(&object);
                fs::remove_file(&path).await?;
                if let Some(parent) = path.parent() {
                    fs::remove_dir(parent).await?;
                }
                Ok(())
            }
            Self::S3(sink) => sink.delete(&object).await,
        }
    }

    pub async fn probe(&self) -> Result<(), SinkError> {
        let export_id = Uuid::new_v4();
        let payload = Bytes::from_static(b"csfx-export-probe");
        self.write(export_id, "probe", payload.clone()).await?;
        let mut stream = self.open(export_id, "probe").await?;
        let mut read = Vec::new();
        while let Some(chunk) = stream.next().await {
            read.extend_from_slice(&chunk.map_err(|e| SinkError::Remote(e.to_string()))?);
        }
        self.remove(export_id, "probe").await?;
        if read == payload.as_ref() {
            Ok(())
        } else {
            Err(SinkError::Remote(
                "probe object read back differently".to_string(),
            ))
        }
    }
}

async fn open_local(path: PathBuf) -> Result<Writer, SinkError> {
    if let Some(parent) = path.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)
            .await?;
    }
    let temporary = path.with_extension("partial");
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&temporary)
        .await?;
    Ok(Writer::Local {
        file,
        temporary,
        path,
    })
}

async fn open_local_stream(path: PathBuf) -> Result<ByteStream, SinkError> {
    let file = fs::File::open(path).await?;
    Ok(Box::pin(stream::try_unfold(file, |mut file| async move {
        let mut buffer = Vec::with_capacity(READ_CHUNK);
        let read = (&mut file)
            .take(READ_CHUNK as u64)
            .read_to_end(&mut buffer)
            .await
            .map_err(|error| ExportError::Sink(error.into()))?;
        Ok((read > 0).then(|| (Bytes::from(buffer), file)))
    })))
}

impl Writer {
    pub async fn write(&mut self, bytes: Bytes) -> Result<(), SinkError> {
        match self {
            Self::Local { file, .. } => Ok(file.write_all(&bytes).await?),
            Self::S3(writer) => writer.write(&bytes).await,
        }
    }

    pub async fn finish(self) -> Result<(), SinkError> {
        match self {
            Self::Local {
                file,
                temporary,
                path,
            } => {
                file.sync_all().await?;
                Ok(fs::rename(temporary, path).await?)
            }
            Self::S3(writer) => writer.finish().await,
        }
    }

    pub async fn abort(self) {
        match self {
            Self::Local { temporary, .. } => {
                if let Err(error) = fs::remove_file(&temporary).await {
                    tracing::warn!(error = %error, "failed to remove partial export file");
                }
            }
            Self::S3(writer) => writer.abort().await,
        }
    }
}

impl S3Writer {
    async fn push_part(&mut self, part: Vec<u8>) -> Result<(), SinkError> {
        if self.upload_id.is_none() {
            self.upload_id = Some(self.sink.start_upload(&self.key).await?);
        }
        let upload_id = self.upload_id.as_deref().unwrap_or_default();
        let number = u32::try_from(self.etags.len() + 1)
            .map_err(|_| SinkError::Remote("too many upload parts".to_string()))?;
        let etag = self
            .sink
            .upload_part(&self.key, upload_id, number, Bytes::from(part))
            .await?;
        self.etags.push(etag);
        Ok(())
    }

    async fn write(&mut self, bytes: &[u8]) -> Result<(), SinkError> {
        self.buffer.extend_from_slice(bytes);
        while self.buffer.len() >= PART_SIZE {
            let part: Vec<u8> = self.buffer.drain(..PART_SIZE).collect();
            self.push_part(part).await?;
        }
        Ok(())
    }

    async fn finish(mut self) -> Result<(), SinkError> {
        let Some(upload_id) = self.upload_id.clone() else {
            return self.sink.put(&self.key, Bytes::from(self.buffer)).await;
        };
        if !self.buffer.is_empty() {
            let rest = std::mem::take(&mut self.buffer);
            self.push_part(rest).await?;
        }
        self.sink
            .complete_upload(&self.key, &upload_id, &self.etags)
            .await
    }

    async fn abort(self) {
        let Some(upload_id) = self.upload_id else {
            return;
        };
        if let Err(error) = self.sink.abort_upload(&self.key, &upload_id).await {
            tracing::warn!(error = %error, "failed to abort multipart upload");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn collect(mut stream: ByteStream) -> Vec<u8> {
        let mut out = Vec::new();
        while let Some(chunk) = stream.next().await {
            out.extend_from_slice(&chunk.unwrap());
        }
        out
    }

    #[tokio::test]
    async fn writes_privately_and_reads_back() {
        let root = std::env::temp_dir().join(format!("csfx-sink-{}", Uuid::new_v4()));
        let sink = Sink::Local(root.clone());
        let id = Uuid::new_v4();
        sink.write(id, "config/role.json", Bytes::from_static(b"[]"))
            .await
            .unwrap();
        let stream = sink.open(id, "config/role.json").await.unwrap();
        assert_eq!(collect(stream).await, b"[]");
        let mode = std::fs::metadata(root.join(id.to_string()).join("config/role.json"))
            .unwrap()
            .permissions();
        assert_eq!(
            std::os::unix::fs::PermissionsExt::mode(&mode) & 0o777,
            0o600
        );
        sink.probe().await.unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn rejects_path_traversal() {
        let sink = Sink::Local(std::env::temp_dir());
        let id = Uuid::new_v4();
        for name in ["../escape", "/etc/passwd", "", "a/../../b"] {
            assert!(matches!(
                sink.write(id, name, Bytes::from_static(b"x")).await,
                Err(SinkError::InvalidName(_))
            ));
        }
    }

    #[tokio::test]
    #[ignore = "requires an S3 endpoint in CSFX_TEST_S3_*"]
    async fn multipart_upload_round_trips_through_s3() {
        use crate::export::{crypto::ExportKey, pipeline};
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
        let var = |name: &str| std::env::var(name).expect(name);
        let sink = Sink::s3(S3Config {
            endpoint: var("CSFX_TEST_S3_ENDPOINT"),
            region: "us-east-1".to_string(),
            bucket: var("CSFX_TEST_S3_BUCKET"),
            prefix: "csfx/multipart".to_string(),
            access_key_id: var("CSFX_TEST_S3_KEY"),
            secret_access_key: var("CSFX_TEST_S3_SECRET"),
        });
        let payload: Vec<u8> = (0..(PART_SIZE * 2 + 12_345) as u32 as usize)
            .map(|n| (n % 251) as u8)
            .collect();
        let (key, _) = ExportKey::create("multipart passphrase").unwrap();
        let id = Uuid::new_v4();
        let stored = pipeline::store(
            &sink,
            id,
            "big/blob.bin",
            Some(&key),
            pipeline::once(payload.clone()),
        )
        .await
        .unwrap();
        let source = sink.open(id, "big/blob.bin").await.unwrap();
        let opened = pipeline::verified(source, Some(&key), stored.sha256, "blob".to_string());
        assert_eq!(pipeline::collect(opened).await.unwrap(), payload);
        sink.remove(id, "big/blob.bin").await.unwrap();
    }
}
