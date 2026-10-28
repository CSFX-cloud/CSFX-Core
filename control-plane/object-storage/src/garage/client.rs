use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone)]
pub struct GarageClient {
    http: reqwest::Client,
    admin_url: String,
    admin_token: String,
}

#[derive(Debug, Deserialize)]
pub struct GarageBucket {
    pub id: String,
}

#[derive(Debug, Serialize)]
struct CreateBucketBody {
    #[serde(rename = "globalAlias")]
    global_alias: String,
}

#[derive(Debug, Serialize)]
struct UpdateBucketBody {
    quotas: UpdateBucketQuotas,
}

#[derive(Debug, Serialize)]
struct UpdateBucketQuotas {
    #[serde(rename = "maxSize", skip_serializing_if = "Option::is_none")]
    max_size: Option<i64>,
    #[serde(rename = "maxObjects", skip_serializing_if = "Option::is_none")]
    max_objects: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GarageKey {
    #[serde(rename = "accessKeyId")]
    pub access_key_id: String,
    #[serde(rename = "secretAccessKey")]
    pub secret_access_key: String,
}

#[derive(Debug, Deserialize)]
pub struct ClusterStatusNode {
    pub id: String,
    #[serde(default)]
    pub addr: Option<String>,
    #[serde(rename = "isUp", default)]
    pub is_up: bool,
    #[serde(default)]
    pub role: Option<ClusterStatusNodeRole>,
}

#[derive(Debug, Deserialize)]
pub struct ClusterStatusNodeRole {
    #[serde(default)]
    pub zone: String,
    #[serde(default)]
    pub capacity: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct NodeInfoResponse {
    success: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct ClusterStatus {
    #[serde(default)]
    pub nodes: Vec<ClusterStatusNode>,
    #[serde(rename = "layoutVersion", default)]
    pub layout_version: i64,
}

#[derive(Debug, Serialize)]
pub struct LayoutRole {
    pub id: String,
    pub zone: String,
    pub capacity: Option<i64>,
    pub tags: Vec<String>,
}

impl GarageClient {
    pub fn new(admin_url: String, admin_token: String) -> Self {
        Self {
            http: reqwest::Client::new(),
            admin_url,
            admin_token,
        }
    }

    fn url(&self, path: &str) -> String {
        format!("{}{}", self.admin_url, path)
    }

    async fn check_status(response: reqwest::Response, context: &str) -> Result<reqwest::Response> {
        if response.status().is_success() {
            return Ok(response);
        }
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        bail!(
            "garage admin api error context={} status={} body={}",
            context,
            status,
            body
        )
    }

    pub async fn create_bucket(&self, global_alias: &str) -> Result<GarageBucket> {
        let response = self
            .http
            .post(self.url("/v2/CreateBucket"))
            .bearer_auth(&self.admin_token)
            .json(&CreateBucketBody {
                global_alias: global_alias.to_string(),
            })
            .send()
            .await
            .context("create_bucket request failed")?;

        let response = Self::check_status(response, "create_bucket").await?;
        response
            .json::<GarageBucket>()
            .await
            .context("failed to parse create_bucket response")
    }

    pub async fn update_bucket_quotas(
        &self,
        garage_bucket_id: &str,
        max_size: Option<i64>,
        max_objects: Option<i64>,
    ) -> Result<()> {
        let response = self
            .http
            .post(self.url(&format!("/v2/UpdateBucket?id={}", garage_bucket_id)))
            .bearer_auth(&self.admin_token)
            .json(&UpdateBucketBody {
                quotas: UpdateBucketQuotas {
                    max_size,
                    max_objects,
                },
            })
            .send()
            .await
            .context("update_bucket_quotas request failed")?;

        Self::check_status(response, "update_bucket_quotas").await?;
        Ok(())
    }

    pub async fn delete_bucket(&self, garage_bucket_id: &str) -> Result<()> {
        let response = self
            .http
            .post(self.url(&format!("/v2/DeleteBucket?id={}", garage_bucket_id)))
            .bearer_auth(&self.admin_token)
            .send()
            .await
            .context("delete_bucket request failed")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }

        Self::check_status(response, "delete_bucket").await?;
        Ok(())
    }

    pub async fn create_key(&self, name: &str) -> Result<GarageKey> {
        let response = self
            .http
            .post(self.url("/v2/CreateKey"))
            .bearer_auth(&self.admin_token)
            .json(&json!({ "name": name }))
            .send()
            .await
            .context("create_key request failed")?;

        let response = Self::check_status(response, "create_key").await?;
        response
            .json::<GarageKey>()
            .await
            .context("failed to parse create_key response")
    }

    pub async fn delete_key(&self, garage_key_id: &str) -> Result<()> {
        let response = self
            .http
            .post(self.url(&format!("/v2/DeleteKey?id={}", garage_key_id)))
            .bearer_auth(&self.admin_token)
            .send()
            .await
            .context("delete_key request failed")?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(());
        }

        Self::check_status(response, "delete_key").await?;
        Ok(())
    }

    pub async fn allow_bucket_key(
        &self,
        garage_bucket_id: &str,
        garage_key_id: &str,
        permissions: &str,
    ) -> Result<()> {
        let (read, write, owner) = match permissions {
            "read" => (true, false, false),
            "readwrite" => (true, true, false),
            "owner" => (true, true, true),
            other => bail!("unknown bucket key permission permissions={}", other),
        };

        let response = self
            .http
            .post(self.url("/v2/AllowBucketKey"))
            .bearer_auth(&self.admin_token)
            .json(&json!({
                "bucketId": garage_bucket_id,
                "accessKeyId": garage_key_id,
                "permissions": { "read": read, "write": write, "owner": owner },
            }))
            .send()
            .await
            .context("allow_bucket_key request failed")?;

        Self::check_status(response, "allow_bucket_key").await?;
        Ok(())
    }

    pub async fn get_cluster_status(&self) -> Result<ClusterStatus> {
        let response = self
            .http
            .get(self.url("/v2/GetClusterStatus"))
            .bearer_auth(&self.admin_token)
            .send()
            .await
            .context("get_cluster_status request failed")?;

        let response = Self::check_status(response, "get_cluster_status").await?;
        response
            .json::<ClusterStatus>()
            .await
            .context("failed to parse get_cluster_status response")
    }

    pub async fn get_self_node_id(&self) -> Result<String> {
        let response = self
            .http
            .get(self.url("/v2/GetNodeInfo?node=self"))
            .bearer_auth(&self.admin_token)
            .send()
            .await
            .context("get_self_node_id request failed")?;

        let response = Self::check_status(response, "get_self_node_id").await?;
        let info = response
            .json::<NodeInfoResponse>()
            .await
            .context("failed to parse get_self_node_id response")?;
        info.success
            .into_keys()
            .next()
            .context("garage returned no node info for self")
    }

    pub async fn connect_cluster_nodes(&self, node_addrs: &[String]) -> Result<()> {
        if node_addrs.is_empty() {
            return Ok(());
        }

        let response = self
            .http
            .post(self.url("/v2/ConnectClusterNodes"))
            .bearer_auth(&self.admin_token)
            .json(node_addrs)
            .send()
            .await
            .context("connect_cluster_nodes request failed")?;

        Self::check_status(response, "connect_cluster_nodes").await?;
        Ok(())
    }

    pub async fn update_cluster_layout(&self, roles: Vec<LayoutRole>) -> Result<()> {
        let response = self
            .http
            .post(self.url("/v2/UpdateClusterLayout"))
            .bearer_auth(&self.admin_token)
            .json(&json!({
                "roles": roles,
                "parameters": { "zoneRedundancy": "maximum" },
            }))
            .send()
            .await
            .context("update_cluster_layout request failed")?;

        Self::check_status(response, "update_cluster_layout").await?;
        Ok(())
    }

    pub async fn apply_cluster_layout(&self, version: i64) -> Result<()> {
        let response = self
            .http
            .post(self.url("/v2/ApplyClusterLayout"))
            .bearer_auth(&self.admin_token)
            .json(&json!({ "version": version }))
            .send()
            .await
            .context("apply_cluster_layout request failed")?;

        Self::check_status(response, "apply_cluster_layout").await?;
        Ok(())
    }
}
