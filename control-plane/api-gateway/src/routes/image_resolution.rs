use axum::{
    extract::{ConnectInfo, Path, State},
    http::StatusCode,
    routing::post,
    Json, Router,
};
use entity::{image_registries, ImageRegistries};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use site_builder::images::registry_prefix;
use std::net::SocketAddr;
use tracing::warn;
use uuid::Uuid;

use crate::{routes::agent_stream::is_internal_source, AppState};

const SITE_IMAGE_HOST: &str = "sites.csfx.invalid";
const DEFAULT_TAG: &str = "latest";
const LOCAL_HOST_NAME: &str = "localhost";

#[derive(Deserialize)]
pub struct ResolveImageRequest {
    image: String,
}

#[derive(Serialize)]
pub struct ResolveImageResponse {
    image: String,
}

struct ImageName<'a> {
    path: &'a str,
    tag: &'a str,
}

fn unqualified_name(image: &str) -> Option<ImageName<'_>> {
    if image.contains('@') {
        return None;
    }
    let last_segment_start = image.rfind('/').map_or(0, |index| index + 1);
    let (path, tag) = match image[last_segment_start..].rfind(':') {
        Some(offset) => {
            let split = last_segment_start + offset;
            (&image[..split], &image[split + 1..])
        }
        None => (image, DEFAULT_TAG),
    };
    let first_segment = path.split('/').next().unwrap_or_default();
    let names_host = path.contains('/')
        && (first_segment.contains('.')
            || first_segment.contains(':')
            || first_segment == LOCAL_HOST_NAME);
    if names_host || path.is_empty() || tag.is_empty() {
        return None;
    }
    Some(ImageName { path, tag })
}

fn listing_contains(listing: &Value, repository: &str, tag: &str) -> bool {
    listing["repositories"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|entry| entry["repository"].as_str() == Some(repository))
        .flat_map(|entry| entry["tags"].as_array().into_iter().flatten())
        .any(|entry| entry["tag"].as_str() == Some(tag))
}

async fn registry_listing(state: &AppState, prefix: &str) -> Option<Value> {
    let builder = state.site_builder.as_ref()?;
    match builder
        .post("/internal/registries/images", &json!({ "prefix": prefix }))
        .await
    {
        Ok(listing) => Some(listing),
        Err(error) => {
            warn!(prefix = %prefix, error = %error, "failed to list registry images for resolution");
            None
        }
    }
}

async fn resolve(state: &AppState, group_id: Uuid, image: &str) -> String {
    let Some(name) = unqualified_name(image) else {
        return image.to_string();
    };
    let registries = match ImageRegistries::find()
        .filter(image_registries::Column::ResourceGroupId.eq(group_id))
        .order_by_asc(image_registries::Column::Name)
        .all(&state.db_conn)
        .await
    {
        Ok(registries) => registries,
        Err(error) => {
            warn!(group_id = %group_id, error = %error, "failed to load registries for resolution");
            return image.to_string();
        }
    };

    for registry in registries {
        let prefix = registry_prefix(group_id, &registry.name);
        let repository = format!("{prefix}{}", name.path);
        let Some(listing) = registry_listing(state, &prefix).await else {
            continue;
        };
        if listing_contains(&listing, &repository, name.tag) {
            return format!("{SITE_IMAGE_HOST}/{repository}:{}", name.tag);
        }
    }
    image.to_string()
}

pub async fn resolve_image_handler(
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    State(state): State<AppState>,
    Path(group_id): Path<Uuid>,
    Json(request): Json<ResolveImageRequest>,
) -> Result<Json<ResolveImageResponse>, StatusCode> {
    if !is_internal_source(&addr) {
        warn!(source = %addr, "rejected image resolution from non-internal source");
        return Err(StatusCode::FORBIDDEN);
    }
    let image = resolve(&state, group_id, &request.image).await;
    Ok(Json(ResolveImageResponse { image }))
}

pub fn image_resolution_internal_routes() -> Router<AppState> {
    Router::new().route(
        "/internal/resource-groups/{group_id}/resolve-image",
        post(resolve_image_handler),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_are_candidates_for_registry_lookup() {
        let name = unqualified_name("volume-test:2").unwrap();
        assert_eq!((name.path, name.tag), ("volume-test", "2"));
        let name = unqualified_name("team/app").unwrap();
        assert_eq!((name.path, name.tag), ("team/app", "latest"));
    }

    #[test]
    fn references_with_a_host_or_digest_are_left_alone() {
        assert!(unqualified_name("ghcr.io/org/app:1").is_none());
        assert!(unqualified_name("localhost:5060/rg/app:1").is_none());
        assert!(unqualified_name("registry:5000/app").is_none());
        assert!(unqualified_name("redis@sha256:abc").is_none());
    }

    #[test]
    fn listing_requires_repository_and_tag() {
        let listing = json!({
            "repositories": [{ "repository": "rg-1/test/app", "tags": [{ "tag": "latest" }] }]
        });
        assert!(listing_contains(&listing, "rg-1/test/app", "latest"));
        assert!(!listing_contains(&listing, "rg-1/test/app", "2"));
        assert!(!listing_contains(&listing, "rg-1/test/other", "latest"));
    }
}
