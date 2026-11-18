use uuid::Uuid;

use super::{
    crypto::{EncryptionInfo, ExportKey},
    manifest::{Artifact, Manifest, Scope, MANIFEST_NAME},
    pipeline,
    sink::Sink,
    target::Target,
    ByteStream, ExportError,
};

pub struct Exporter {
    pub export_id: Uuid,
    sink: Sink,
    key: Option<ExportKey>,
    pub encryption: Option<EncryptionInfo>,
    artifacts: Vec<Artifact>,
}

impl Exporter {
    pub fn new(export_id: Uuid, target: Target) -> Result<Self, ExportError> {
        let (key, encryption) = match target.passphrase.as_deref() {
            Some(passphrase) => {
                let (key, info) = ExportKey::create(passphrase)?;
                (Some(key), Some(info))
            }
            None => (None, None),
        };
        Ok(Self {
            export_id,
            sink: target.sink,
            key,
            encryption,
            artifacts: Vec::new(),
        })
    }

    pub async fn add_stream(
        &mut self,
        name: String,
        kind: &str,
        source_id: Option<Uuid>,
        source: ByteStream,
    ) -> Result<(), ExportError> {
        let stored =
            pipeline::store(&self.sink, self.export_id, &name, self.key.as_ref(), source).await?;
        self.artifacts.push(Artifact {
            name,
            kind: kind.to_string(),
            source_id,
            bytes: stored.bytes,
            sha256: stored.sha256,
        });
        Ok(())
    }

    pub async fn add_bytes(
        &mut self,
        name: String,
        kind: &str,
        source_id: Option<Uuid>,
        bytes: Vec<u8>,
    ) -> Result<(), ExportError> {
        self.add_stream(name, kind, source_id, pipeline::once(bytes))
            .await
    }

    pub async fn finish(
        self,
        schema_migration: String,
        scope: Scope,
    ) -> Result<Manifest, ExportError> {
        let manifest = Manifest::new(
            self.export_id,
            schema_migration,
            scope,
            self.encryption,
            self.artifacts,
        );
        let encoded = serde_json::to_vec_pretty(&manifest)?;
        self.sink
            .write(self.export_id, MANIFEST_NAME, encoded.into())
            .await?;
        Ok(manifest)
    }
}

pub struct Reader {
    sink: Sink,
    key: Option<ExportKey>,
    pub manifest: Manifest,
}

impl Reader {
    pub async fn open(
        target: Target,
        export_id: Uuid,
        passphrase: Option<String>,
    ) -> Result<Self, ExportError> {
        let source = target.sink.open(export_id, MANIFEST_NAME).await?;
        let manifest: Manifest = serde_json::from_slice(&pipeline::collect(source).await?)?;
        if !manifest.supported_format() || manifest.export_id != export_id {
            return Err(ExportError::UnsupportedFormat);
        }
        let key = match (&manifest.encryption, passphrase.or(target.passphrase)) {
            (None, _) => None,
            (Some(_), None) => return Err(ExportError::PassphraseRequired),
            (Some(info), Some(passphrase)) => Some(ExportKey::open(&passphrase, info)?),
        };
        Ok(Self {
            sink: target.sink,
            key,
            manifest,
        })
    }

    pub fn artifact(&self, name: &str) -> Result<&Artifact, ExportError> {
        self.manifest
            .artifacts
            .iter()
            .find(|artifact| artifact.name == name)
            .ok_or_else(|| ExportError::MissingArtifact(name.to_string()))
    }

    pub async fn stream(&self, name: &str) -> Result<ByteStream, ExportError> {
        let artifact = self.artifact(name)?;
        let source = self.sink.open(self.manifest.export_id, name).await?;
        Ok(pipeline::verified(
            source,
            self.key.as_ref(),
            artifact.sha256.clone(),
            name.to_string(),
        ))
    }

    pub async fn bytes(&self, name: &str) -> Result<Vec<u8>, ExportError> {
        pipeline::collect(self.stream(name).await?).await
    }
}
