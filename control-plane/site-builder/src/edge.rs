use crate::artifacts::{self, Archive};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use entity::{site_deployments, site_domains, SiteDeployments, SiteDomains};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const HOST_TTL: Duration = Duration::from_secs(5);
const MAX_CACHED_ARCHIVES: usize = 16;
const INDEX_FILE: &str = "index.html";
const NOT_FOUND_FILE: &str = "404.html";
const REVALIDATE: &str = "public, max-age=0, must-revalidate";
const IMMUTABLE_ASSET: &str = "public, max-age=3600";

pub struct Edge {
    db: DatabaseConnection,
    archives: Mutex<HashMap<Uuid, Arc<Archive>>>,
    hosts: Mutex<HashMap<String, (Option<Uuid>, Instant)>>,
}

pub struct Located<'a> {
    pub path: &'a str,
    pub data: &'a [u8],
    pub status: StatusCode,
}

pub fn locate<'a>(archive: &'a Archive, request_path: &str) -> Option<Located<'a>> {
    let trimmed = request_path.trim_matches('/');
    let candidates = [
        trimmed.to_string(),
        format!("{trimmed}/{INDEX_FILE}"),
        format!("{trimmed}.html"),
    ];
    let directory = trimmed.is_empty() || request_path.ends_with('/');
    let tries: Vec<String> = if directory {
        vec![format!("{trimmed}/{INDEX_FILE}")
            .trim_start_matches('/')
            .to_string()]
    } else {
        candidates.to_vec()
    };
    for candidate in &tries {
        if let Some((path, data)) = archive.files.get_key_value(candidate.as_str()) {
            return Some(Located {
                path,
                data,
                status: StatusCode::OK,
            });
        }
    }
    fallback(archive, trimmed)
}

fn fallback<'a>(archive: &'a Archive, trimmed: &str) -> Option<Located<'a>> {
    if let Some((path, data)) = archive.files.get_key_value(NOT_FOUND_FILE) {
        return Some(Located {
            path,
            data,
            status: StatusCode::NOT_FOUND,
        });
    }
    let has_extension = trimmed
        .rsplit('/')
        .next()
        .is_some_and(|name| name.contains('.'));
    if has_extension {
        return None;
    }
    let (path, data) = archive.files.get_key_value(INDEX_FILE)?;
    Some(Located {
        path,
        data,
        status: StatusCode::OK,
    })
}

impl Edge {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            archives: Mutex::new(HashMap::new()),
            hosts: Mutex::new(HashMap::new()),
        }
    }

    async fn deployment_for_host(
        &self,
        host: &str,
    ) -> Result<Option<Uuid>, crate::error::ServiceError> {
        if let Some(cached) = self.cached_host(host) {
            return Ok(cached);
        }
        let resolved = self.resolve_host(host).await?;
        self.hosts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(host.to_string(), (resolved, Instant::now()));
        Ok(resolved)
    }

    fn cached_host(&self, host: &str) -> Option<Option<Uuid>> {
        let hosts = self
            .hosts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let (resolved, stored) = hosts.get(host)?;
        (stored.elapsed() < HOST_TTL).then_some(*resolved)
    }

    async fn resolve_host(&self, host: &str) -> Result<Option<Uuid>, crate::error::ServiceError> {
        let Some(domain) = SiteDomains::find()
            .filter(site_domains::Column::Hostname.eq(host))
            .one(&self.db)
            .await?
        else {
            return Ok(None);
        };
        let site = crate::sites::load(&self.db, domain.site_id).await?;
        if site.mode != "static" {
            return Ok(None);
        }
        if domain.kind != "branch" {
            return Ok(site.current_deployment_id);
        }
        Ok(SiteDeployments::find()
            .filter(site_deployments::Column::SiteId.eq(site.id))
            .filter(site_deployments::Column::Status.eq("ready"))
            .filter(site_deployments::Column::Branch.eq(domain.branch.unwrap_or_default()))
            .order_by_desc(site_deployments::Column::CreatedAt)
            .one(&self.db)
            .await?
            .map(|deployment| deployment.id))
    }

    async fn archive(&self, id: Uuid) -> Result<Option<Arc<Archive>>, crate::error::ServiceError> {
        if let Some(found) = self
            .archives
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .get(&id)
        {
            return Ok(Some(found.clone()));
        }
        let Some(loaded) = artifacts::load(&self.db, id).await? else {
            return Ok(None);
        };
        let shared = Arc::new(loaded);
        let mut cache = self.archives.lock().unwrap_or_else(|p| p.into_inner());
        if cache.len() >= MAX_CACHED_ARCHIVES {
            let evicted = cache.keys().next().copied();
            if let Some(evicted) = evicted {
                cache.remove(&evicted);
            }
        }
        cache.insert(id, shared.clone());
        Ok(Some(shared))
    }
}

fn host_of(headers: &HeaderMap) -> Option<String> {
    let raw = headers.get(header::HOST)?.to_str().ok()?;
    let name = raw
        .split(':')
        .next()?
        .trim_end_matches('.')
        .to_ascii_lowercase();
    (!name.is_empty()).then_some(name)
}

fn respond(located: Located<'_>, deployment: Uuid) -> Response {
    let mime = mime_guess::from_path(located.path).first_or_octet_stream();
    let cache = if mime.subtype() == "html" {
        REVALIDATE
    } else {
        IMMUTABLE_ASSET
    };
    (
        located.status,
        [
            (header::CONTENT_TYPE, mime.essence_str().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff".to_string()),
            (
                header::HeaderName::from_static("x-csfx-deployment"),
                deployment.to_string(),
            ),
        ],
        located.data.to_vec(),
    )
        .into_response()
}

pub async fn serve(State(edge): State<Arc<Edge>>, headers: HeaderMap, uri: Uri) -> Response {
    let Some(host) = host_of(&headers) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let outcome = async {
        let Some(id) = edge.deployment_for_host(&host).await? else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        let Some(archive) = edge.archive(id).await? else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        Ok::<Response, crate::error::ServiceError>(match locate(&archive, uri.path()) {
            Some(located) => respond(located, id),
            None => StatusCode::NOT_FOUND.into_response(),
        })
    }
    .await;
    outcome.unwrap_or_else(|error| {
        tracing::error!(host = %host, error = %error, "edge request failed");
        StatusCode::BAD_GATEWAY.into_response()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn archive(names: &[&str]) -> Archive {
        Archive {
            files: names
                .iter()
                .map(|n| (n.to_string(), n.as_bytes().to_vec()))
                .collect(),
        }
    }

    #[test]
    fn root_and_directories_resolve_to_index_files() {
        let site = archive(&["index.html", "docs/index.html"]);
        assert_eq!(locate(&site, "/").unwrap().path, "index.html");
        assert_eq!(locate(&site, "/docs/").unwrap().path, "docs/index.html");
        assert_eq!(locate(&site, "/docs").unwrap().path, "docs/index.html");
    }

    #[test]
    fn clean_urls_resolve_to_html_files() {
        let site = archive(&["index.html", "about.html"]);
        assert_eq!(locate(&site, "/about").unwrap().path, "about.html");
    }

    #[test]
    fn missing_pages_use_404_page_or_spa_fallback() {
        let with_404 = archive(&["index.html", "404.html"]);
        let found = locate(&with_404, "/nope").unwrap();
        assert_eq!(found.path, "404.html");
        assert_eq!(found.status, StatusCode::NOT_FOUND);

        let spa = archive(&["index.html"]);
        assert_eq!(locate(&spa, "/dashboard/x").unwrap().status, StatusCode::OK);
        assert!(locate(&spa, "/missing.png").is_none());
    }

    #[test]
    fn host_header_is_normalized() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "Shop.CSFX.dev:443".parse().unwrap());
        assert_eq!(host_of(&headers).as_deref(), Some("shop.csfx.dev"));
    }
}
