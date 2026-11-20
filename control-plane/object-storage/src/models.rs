use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateBucketRequest {
    pub name: String,
    pub exposure: Option<String>,
    pub quota_max_size: Option<i64>,
    pub quota_max_objects: Option<i64>,
    pub organization_id: Option<Uuid>,
    pub resource_group_id: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateBucketRequest {
    pub exposure: Option<String>,
    pub quota_max_size: Option<i64>,
    pub quota_max_objects: Option<i64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAccessKeyRequest {
    pub name: String,
    pub permissions: Option<String>,
    pub expires_at: Option<chrono::NaiveDateTime>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessKeyResponse {
    pub id: Uuid,
    pub bucket_id: Uuid,
    pub name: String,
    pub garage_key_id: String,
    pub permissions: String,
    pub expires_at: Option<chrono::NaiveDateTime>,
    pub last_rotated_at: Option<chrono::NaiveDateTime>,
    pub created_at: chrono::NaiveDateTime,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AccessKeyCreatedResponse {
    #[serde(flatten)]
    pub key: AccessKeyResponse,
    pub secret_access_key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ObjectEntry {
    pub key: String,
    pub size: i64,
    pub last_modified: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListObjectsResponse {
    pub objects: Vec<ObjectEntry>,
    pub folders: Vec<String>,
    pub next_continuation_token: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ListObjectsQuery {
    #[serde(default)]
    pub prefix: String,
    #[serde(default)]
    pub continuation_token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PresignQuery {
    pub origin: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PresignUploadRequest {
    pub key: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PresignResponse {
    pub url: String,
    pub expires_in_seconds: u64,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClusterStatusResponse {
    pub storage_node_count: u32,
    pub replication_factor: u32,
    pub degraded: bool,
    pub nodes: Vec<entity::entities::garage_nodes::Model>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct BucketResponse {
    pub id: Uuid,
    pub name: String,
    pub global_alias: String,
    pub exposure: String,
    pub quota_max_size: Option<i64>,
    pub quota_max_objects: Option<i64>,
    pub status: String,
    pub organization_id: Option<Uuid>,
    pub resource_group_id: Option<Uuid>,
    pub created_at: chrono::NaiveDateTime,
    pub updated_at: Option<chrono::NaiveDateTime>,
}
