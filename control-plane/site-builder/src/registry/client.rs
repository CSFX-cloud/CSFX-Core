use super::{keys::RegistryKeys, sign, AccessEntry};
use crate::error::ServiceError;
use reqwest::{header, Client, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const SERVICE_ACCOUNT: &str = "csfx-site-builder";
const DIGEST_HEADER: &str = "Docker-Content-Digest";
const MANIFEST_ACCEPT: &str = "application/vnd.oci.image.index.v1+json, application/vnd.oci.image.manifest.v1+json, application/vnd.docker.distribution.manifest.list.v2+json, application/vnd.docker.distribution.manifest.v2+json";

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct ImageTag {
    pub tag: String,
    pub digest: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct RepositoryImages {
    pub repository: String,
    pub tags: Vec<ImageTag>,
}

#[derive(Deserialize)]
struct Catalog {
    repositories: Option<Vec<String>>,
}

const CATALOG_PAGE_SIZE: u32 = 1000;

pub fn manifest_size(manifest: &Value) -> Option<u64> {
    let config = manifest["config"]["size"].as_u64()?;
    let layers: Option<Vec<u64>> = manifest["layers"]
        .as_array()?
        .iter()
        .map(|layer| layer["size"].as_u64())
        .collect();
    Some(config + layers?.iter().sum::<u64>())
}

pub struct RegistryClient {
    http: Client,
    base: String,
    keys: Arc<RegistryKeys>,
}

#[derive(Deserialize)]
struct TagList {
    tags: Option<Vec<String>>,
}

fn failure(error: impl std::fmt::Display) -> ServiceError {
    ServiceError::Registry(error.to_string())
}

pub fn base_url(registry: &str, insecure: bool) -> String {
    let scheme = if insecure { "http" } else { "https" };
    format!("{scheme}://{}", registry.trim_end_matches('/'))
}

impl RegistryClient {
    pub fn new(
        registry: &str,
        insecure: bool,
        keys: Arc<RegistryKeys>,
    ) -> Result<Self, ServiceError> {
        Ok(Self {
            http: Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .map_err(failure)?,
            base: base_url(registry, insecure),
            keys,
        })
    }

    fn catalog_token(&self) -> Result<String, ServiceError> {
        let access = vec![AccessEntry {
            kind: "repository".to_string(),
            name: String::new(),
            actions: vec!["pull".to_string()],
        }];
        sign(&self.keys, SERVICE_ACCOUNT.to_string(), access)
    }

    fn token(&self, repository: &str) -> Result<String, ServiceError> {
        let access = vec![AccessEntry {
            kind: "repository".to_string(),
            name: repository.to_string(),
            actions: ["pull", "delete", "push"].map(str::to_string).to_vec(),
        }];
        sign(&self.keys, SERVICE_ACCOUNT.to_string(), access)
    }

    async fn digest(
        &self,
        repository: &str,
        tag: &str,
        token: &str,
    ) -> Result<Option<String>, ServiceError> {
        let response = self
            .http
            .head(format!("{}/v2/{repository}/manifests/{tag}", self.base))
            .bearer_auth(token)
            .header(header::ACCEPT, MANIFEST_ACCEPT)
            .send()
            .await
            .map_err(failure)?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(None),
            status if status.is_success() => Ok(response
                .headers()
                .get(DIGEST_HEADER)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string)),
            status => Err(failure(format!("manifest lookup returned {status}"))),
        }
    }

    pub async fn delete_tag(&self, repository: &str, tag: &str) -> Result<(), ServiceError> {
        let token = self.token(repository)?;
        let Some(digest) = self.digest(repository, tag, &token).await? else {
            return Ok(());
        };
        let response = self
            .http
            .delete(format!("{}/v2/{repository}/manifests/{digest}", self.base))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(failure)?;
        match response.status() {
            StatusCode::NOT_FOUND => Ok(()),
            status if status.is_success() => Ok(()),
            status => Err(failure(format!("manifest delete returned {status}"))),
        }
    }

    pub async fn tags(&self, repository: &str) -> Result<Vec<String>, ServiceError> {
        let token = self.token(repository)?;
        let response = self
            .http
            .get(format!("{}/v2/{repository}/tags/list", self.base))
            .bearer_auth(&token)
            .send()
            .await
            .map_err(failure)?;
        if response.status() == StatusCode::NOT_FOUND {
            return Ok(Vec::new());
        }
        let list: TagList = response
            .error_for_status()
            .map_err(failure)?
            .json()
            .await
            .map_err(failure)?;
        Ok(list.tags.unwrap_or_default())
    }

    pub async fn repositories(&self, prefix: &str) -> Result<Vec<String>, ServiceError> {
        let response = self
            .http
            .get(format!("{}/v2/_catalog?n={CATALOG_PAGE_SIZE}", self.base))
            .bearer_auth(self.catalog_token()?)
            .send()
            .await
            .map_err(failure)?;
        let catalog: Catalog = response
            .error_for_status()
            .map_err(failure)?
            .json()
            .await
            .map_err(failure)?;
        let mut repositories: Vec<String> = catalog
            .repositories
            .unwrap_or_default()
            .into_iter()
            .filter(|name| name.starts_with(prefix))
            .collect();
        repositories.sort();
        Ok(repositories)
    }

    async fn tag_details(&self, repository: &str, tag: &str) -> Result<ImageTag, ServiceError> {
        let token = self.token(repository)?;
        let response = self
            .http
            .get(format!("{}/v2/{repository}/manifests/{tag}", self.base))
            .bearer_auth(&token)
            .header(header::ACCEPT, MANIFEST_ACCEPT)
            .send()
            .await
            .map_err(failure)?
            .error_for_status()
            .map_err(failure)?;
        let digest = response
            .headers()
            .get(DIGEST_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let manifest: Value = response.json().await.map_err(failure)?;
        Ok(ImageTag {
            tag: tag.to_string(),
            digest,
            size_bytes: manifest_size(&manifest),
        })
    }

    pub async fn images(&self, prefix: &str) -> Result<Vec<RepositoryImages>, ServiceError> {
        let mut result = Vec::new();
        for repository in self.repositories(prefix).await? {
            let mut tags = Vec::new();
            for tag in self.tags(&repository).await? {
                tags.push(self.tag_details(&repository, &tag).await?);
            }
            result.push(RepositoryImages { repository, tags });
        }
        Ok(result)
    }

    pub async fn purge_prefix(&self, prefix: &str) -> Result<(), ServiceError> {
        for repository in self.repositories(prefix).await? {
            self.delete_repository(&repository).await?;
        }
        Ok(())
    }

    pub async fn delete_repository(&self, repository: &str) -> Result<(), ServiceError> {
        for tag in self.tags(repository).await? {
            self.delete_tag(repository, &tag).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_sizes_add_the_config_and_all_layers() {
        let manifest = serde_json::json!({
            "config": { "size": 100 },
            "layers": [{ "size": 1000 }, { "size": 20 }]
        });
        assert_eq!(manifest_size(&manifest), Some(1120));
        assert_eq!(manifest_size(&serde_json::json!({ "manifests": [] })), None);
    }

    #[test]
    fn base_urls_follow_the_transport_setting() {
        assert_eq!(base_url("10.0.0.5:5060", true), "http://10.0.0.5:5060");
        assert_eq!(
            base_url("reg.example.com/", false),
            "https://reg.example.com"
        );
    }
}
