use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::crypto::EncryptionInfo;

pub const MANIFEST_NAME: &str = "manifest.json";
const FORMAT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeKind {
    Config,
    Full,
    ResourceGroup,
    Workload,
    Volume,
    Bucket,
}

impl ScopeKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Config => "config",
            Self::Full => "full",
            Self::ResourceGroup => "resource_group",
            Self::Workload => "workload",
            Self::Volume => "volume",
            Self::Bucket => "bucket",
        }
    }

    pub fn needs_id(self) -> bool {
        !matches!(self, Self::Config | Self::Full)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scope {
    pub kind: ScopeKind,
    pub id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub name: String,
    pub kind: String,
    pub source_id: Option<Uuid>,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub format: u32,
    pub csfx_version: String,
    pub schema_migration: String,
    pub export_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub scope: Scope,
    pub consistency: String,
    pub encryption: Option<EncryptionInfo>,
    pub artifacts: Vec<Artifact>,
}

impl Manifest {
    pub fn new(
        export_id: Uuid,
        schema_migration: String,
        scope: Scope,
        encryption: Option<EncryptionInfo>,
        artifacts: Vec<Artifact>,
    ) -> Self {
        Self {
            format: FORMAT_VERSION,
            csfx_version: env!("CARGO_PKG_VERSION").to_string(),
            schema_migration,
            export_id,
            created_at: Utc::now(),
            scope,
            consistency: "crash".to_string(),
            encryption,
            artifacts,
        }
    }

    pub fn supported_format(&self) -> bool {
        self.format == FORMAT_VERSION
    }
}

pub fn hex(digest: &[u8]) -> String {
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn manifest_round_trips() {
        let manifest = Manifest::new(
            Uuid::new_v4(),
            "m1".to_string(),
            Scope {
                kind: ScopeKind::Config,
                id: None,
            },
            None,
            vec![Artifact {
                name: "config/role.json".to_string(),
                kind: "table".to_string(),
                source_id: None,
                bytes: 2,
                sha256: sha256_hex(b"[]"),
            }],
        );
        let parsed: Manifest =
            serde_json::from_slice(&serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert_eq!(parsed.export_id, manifest.export_id);
        assert_eq!(parsed.scope.kind, ScopeKind::Config);
        assert!(parsed.supported_format());
    }
}
