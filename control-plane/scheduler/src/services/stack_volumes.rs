use entity::entities::volumes;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

use crate::models::compose::{ComposeServiceSpec, ComposeVolumeMount};

const VOLUME_MANAGER_URL_ENV: &str = "VOLUME_MANAGER_URL";
const DEFAULT_VOLUME_MANAGER_URL: &str = "http://localhost:8003";
const CREATE_TIMEOUT: Duration = Duration::from_secs(60);
const DEFAULT_VOLUME_SIZE_GB: i32 = 5;

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(CREATE_TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

pub fn volume_name(stack_name: &str, mount: &ComposeVolumeMount) -> String {
    mount
        .external_name
        .clone()
        .unwrap_or_else(|| format!("{}-{}", stack_name, mount.source))
}

pub fn required_mounts<'a>(services: &'a [ComposeServiceSpec]) -> Vec<&'a ComposeVolumeMount> {
    let mut mounts: Vec<&ComposeVolumeMount> = Vec::new();
    for mount in services.iter().flat_map(|service| &service.volumes) {
        if !mounts.iter().any(|known| known.source == mount.source) {
            mounts.push(mount);
        }
    }
    mounts
}

async fn find_volume(
    db: &DatabaseConnection,
    resource_group_id: Uuid,
    name: &str,
) -> Result<Option<Uuid>, String> {
    volumes::Entity::find()
        .filter(volumes::Column::ResourceGroupId.eq(resource_group_id))
        .filter(volumes::Column::Name.eq(name))
        .one(db)
        .await
        .map(|found| found.map(|volume| volume.id))
        .map_err(|e| format!("Failed to look up volume {}: {}", name, e))
}

async fn create_volume(resource_group_id: Uuid, name: &str) -> Result<Uuid, String> {
    let base_url = std::env::var(VOLUME_MANAGER_URL_ENV)
        .unwrap_or_else(|_| DEFAULT_VOLUME_MANAGER_URL.to_string());

    let response = http_client()
        .post(format!("{}/volumes", base_url))
        .json(&serde_json::json!({
            "name": name,
            "size_gb": DEFAULT_VOLUME_SIZE_GB,
            "resource_group_id": resource_group_id,
        }))
        .send()
        .await
        .and_then(|response| response.error_for_status())
        .map_err(|e| format!("Failed to create volume {}: {}", name, e))?;

    let body: serde_json::Value = response
        .json()
        .await
        .map_err(|e| format!("Invalid volume response for {}: {}", name, e))?;
    body["id"]
        .as_str()
        .and_then(|id| Uuid::parse_str(id).ok())
        .ok_or_else(|| format!("Volume response for {} has no id", name))
}

pub async fn ensure(
    db: &DatabaseConnection,
    resource_group_id: Uuid,
    stack_name: &str,
    services: &[ComposeServiceSpec],
) -> Result<HashMap<String, Uuid>, String> {
    let mut resolved = HashMap::new();

    for mount in required_mounts(services) {
        let name = volume_name(stack_name, mount);
        let existing = find_volume(db, resource_group_id, &name).await?;
        let volume_id = match (existing, mount.external_name.is_some()) {
            (Some(id), _) => id,
            (None, true) => return Err(format!("External volume {} does not exist", name)),
            (None, false) => create_volume(resource_group_id, &name).await?,
        };
        resolved.insert(mount.source.clone(), volume_id);
    }

    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(source: &str, external: Option<&str>) -> ComposeVolumeMount {
        ComposeVolumeMount {
            source: source.to_string(),
            external_name: external.map(str::to_string),
            mount_path: "/data".to_string(),
        }
    }

    #[test]
    fn project_volumes_are_prefixed_with_the_stack_name() {
        assert_eq!(volume_name("shop", &mount("db", None)), "shop-db");
    }

    #[test]
    fn external_volumes_keep_their_own_name() {
        assert_eq!(volume_name("shop", &mount("db", Some("shared"))), "shared");
    }
}
