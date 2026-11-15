use crate::artifacts::{self, Archive, Variant};
use crate::edge_metrics;
use crate::encoding::{self, Encoding};
use crate::usage::UsageCounter;
use axum::{
    body::{Body, Bytes},
    extract::State,
    http::{header, HeaderMap, StatusCode, Uri},
    response::{IntoResponse, Response},
};
use chrono::Utc;
use entity::{site_deployments, site_domains, SiteDeployments, SiteDomains};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use uuid::Uuid;

const HOST_TTL: Duration = Duration::from_secs(5);
const MAX_CACHED_HOSTS: usize = 10_000;
const MAX_CACHED_BYTES: usize = 512 * 1024 * 1024;
const INDEX_FILE: &str = "index.html";
const NOT_FOUND_FILE: &str = "404.html";
const REVALIDATE: &str = "public, max-age=0, must-revalidate";
const SHORT_LIVED_ASSET: &str = "public, max-age=3600";
const FINGERPRINTED_ASSET: &str = "public, max-age=31536000, immutable";

pub struct ArchiveCache {
    entries: HashMap<Uuid, (Arc<Archive>, u64)>,
    bytes: usize,
    budget: usize,
    clock: u64,
}

impl ArchiveCache {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            budget,
            clock: 0,
        }
    }

    fn tick(&mut self) -> u64 {
        self.clock += 1;
        self.clock
    }

    pub fn get(&mut self, id: &Uuid) -> Option<Arc<Archive>> {
        let used = self.tick();
        let (archive, last_used) = self.entries.get_mut(id)?;
        *last_used = used;
        Some(archive.clone())
    }

    pub fn insert(&mut self, id: Uuid, archive: Arc<Archive>) {
        let size = archive.size();
        if size > self.budget {
            return;
        }
        while self.bytes + size > self.budget {
            let Some(evicted) = self.least_recently_used() else {
                break;
            };
            if let Some((removed, _)) = self.entries.remove(&evicted) {
                self.bytes -= removed.size();
            }
        }
        let used = self.tick();
        if let Some((replaced, _)) = self.entries.insert(id, (archive, used)) {
            self.bytes -= replaced.size();
        }
        self.bytes += size;
    }

    fn least_recently_used(&self) -> Option<Uuid> {
        self.entries
            .iter()
            .min_by_key(|(_, (_, last_used))| *last_used)
            .map(|(id, _)| *id)
    }
}

#[derive(Default)]
struct HostCache {
    entries: HashMap<String, (Option<Resolved>, Instant)>,
}

impl HostCache {
    fn get(&self, host: &str) -> Option<Option<Resolved>> {
        let (resolved, stored) = self.entries.get(host)?;
        (stored.elapsed() < HOST_TTL).then_some(*resolved)
    }

    fn insert(&mut self, host: String, resolved: Option<Resolved>) {
        if self.entries.len() >= MAX_CACHED_HOSTS {
            self.entries
                .retain(|_, (_, stored)| stored.elapsed() < HOST_TTL);
        }
        if self.entries.len() >= MAX_CACHED_HOSTS {
            self.entries.clear();
        }
        self.entries.insert(host, (resolved, Instant::now()));
    }
}

pub struct Edge {
    db: DatabaseConnection,
    archives: Mutex<ArchiveCache>,
    hosts: Mutex<HostCache>,
    usage: UsageCounter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Resolved {
    site_id: Uuid,
    deployment_id: Uuid,
}

pub struct Located<'a> {
    pub path: &'a str,
    pub data: &'a Bytes,
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
    pub async fn warm(&self, deployment_id: Uuid) {
        if let Err(error) = self.archive(deployment_id).await {
            tracing::warn!(deployment = %deployment_id, error = %error, "edge warm-up failed");
        }
    }

    pub async fn flush_usage(&self) {
        if let Err(error) = self.usage.flush(&self.db).await {
            tracing::warn!(error = %error, "usage flush failed");
        }
    }

    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            db,
            archives: Mutex::new(ArchiveCache::new(MAX_CACHED_BYTES)),
            hosts: Mutex::new(HostCache::default()),
            usage: UsageCounter::default(),
        }
    }

    async fn resolved_host(
        &self,
        host: &str,
    ) -> Result<Option<Resolved>, crate::error::ServiceError> {
        let cached = self.cached_host(host);
        edge_metrics::cache_lookup("host", cached.is_some());
        if let Some(cached) = cached {
            return Ok(cached);
        }
        let resolved = self.resolve_host(host).await?;
        self.hosts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(host.to_string(), resolved);
        Ok(resolved)
    }

    fn cached_host(&self, host: &str) -> Option<Option<Resolved>> {
        self.hosts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(host)
    }

    async fn resolve_host(
        &self,
        host: &str,
    ) -> Result<Option<Resolved>, crate::error::ServiceError> {
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
        let site_id = site.id;
        let resolved = |deployment_id| Resolved {
            site_id,
            deployment_id,
        };
        if domain.kind != "branch" {
            return Ok(site.current_deployment_id.map(resolved));
        }
        Ok(SiteDeployments::find()
            .filter(site_deployments::Column::SiteId.eq(site.id))
            .filter(site_deployments::Column::Status.eq("ready"))
            .filter(site_deployments::Column::Branch.eq(domain.branch.unwrap_or_default()))
            .order_by_desc(site_deployments::Column::CreatedAt)
            .one(&self.db)
            .await?
            .map(|deployment| resolved(deployment.id)))
    }

    async fn archive(&self, id: Uuid) -> Result<Option<Arc<Archive>>, crate::error::ServiceError> {
        let cached = self
            .archives
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .get(&id);
        edge_metrics::cache_lookup("archive", cached.is_some());
        if let Some(found) = cached {
            return Ok(Some(found));
        }
        let Some(loaded) = artifacts::load(&self.db, id).await? else {
            return Ok(None);
        };
        let shared = Arc::new(loaded);
        self.archives
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(id, shared.clone());
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

fn cache_policy(path: &str, html: bool) -> &'static str {
    if html {
        REVALIDATE
    } else if encoding::is_fingerprinted(path) {
        FINGERPRINTED_ASSET
    } else {
        SHORT_LIVED_ASSET
    }
}

fn matches_etag(request: &HeaderMap, etag: &str) -> bool {
    request
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| {
            value
                .split(',')
                .map(|candidate| candidate.trim().trim_start_matches("W/"))
                .any(|candidate| candidate == "*" || candidate == etag)
        })
}

fn variant_for(
    archive: &Archive,
    located: &Located<'_>,
    request: &HeaderMap,
    compressible: bool,
) -> Variant {
    let accepted = request
        .get(header::ACCEPT_ENCODING)
        .and_then(|value| value.to_str().ok());
    let wanted = if compressible {
        encoding::negotiate(accepted)
    } else {
        Encoding::Identity
    };
    archive
        .variant(located.path, wanted)
        .unwrap_or_else(|| Variant {
            data: located.data.clone(),
            etag: format!("\"{}\"", encoding::content_hash(located.data)),
            encoding: Encoding::Identity,
        })
}

fn respond(
    archive: &Archive,
    located: Located<'_>,
    deployment: Uuid,
    request: &HeaderMap,
) -> Response {
    let mime = mime_guess::from_path(located.path).first_or_octet_stream();
    let compressible = encoding::is_compressible(&mime);
    let variant = variant_for(archive, &located, request, compressible);
    let status = if located.status == StatusCode::OK && matches_etag(request, &variant.etag) {
        StatusCode::NOT_MODIFIED
    } else {
        located.status
    };
    if status != StatusCode::NOT_MODIFIED {
        edge_metrics::content_served(variant.encoding, located.data.len(), variant.data.len());
    }
    let mut response = Response::new(if status == StatusCode::NOT_MODIFIED {
        Body::empty()
    } else {
        Body::from(variant.data)
    });
    *response.status_mut() = status;
    let headers = response.headers_mut();
    let mut set = |name: header::HeaderName, value: &str| {
        if let Ok(value) = value.parse() {
            headers.insert(name, value);
        }
    };
    set(header::CONTENT_TYPE, mime.essence_str());
    set(
        header::CACHE_CONTROL,
        cache_policy(located.path, mime.subtype() == "html"),
    );
    set(header::X_CONTENT_TYPE_OPTIONS, "nosniff");
    set(header::ETAG, &variant.etag);
    set(
        header::HeaderName::from_static("x-csfx-deployment"),
        &deployment.to_string(),
    );
    if compressible {
        set(header::VARY, "Accept-Encoding");
    }
    if let Some(token) = variant.encoding.token() {
        set(header::CONTENT_ENCODING, token);
    }
    response
}

pub async fn serve(State(edge): State<Arc<Edge>>, headers: HeaderMap, uri: Uri) -> Response {
    let Some(host) = host_of(&headers) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let outcome = async {
        let Some(resolved) = edge.resolved_host(&host).await? else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        let Some(archive) = edge.archive(resolved.deployment_id).await? else {
            return Ok(StatusCode::NOT_FOUND.into_response());
        };
        Ok::<Response, crate::error::ServiceError>(match locate(&archive, uri.path()) {
            Some(located) => {
                edge.usage
                    .record(resolved.site_id, located.data.len(), Utc::now());
                respond(&archive, located, resolved.deployment_id, &headers)
            }
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
        Archive::new(
            names
                .iter()
                .map(|n| (n.to_string(), Bytes::from(n.as_bytes().to_vec())))
                .collect(),
        )
    }

    fn sized(bytes: usize) -> Arc<Archive> {
        Arc::new(Archive::new(HashMap::from([(
            "index.html".to_string(),
            Bytes::from(vec![0; bytes]),
        )])))
    }

    #[test]
    fn cache_evicts_until_a_new_archive_fits_the_byte_budget() {
        let mut cache = ArchiveCache::new(100);
        cache.insert(Uuid::from_u128(1), sized(60));
        cache.insert(Uuid::from_u128(2), sized(60));
        assert!(cache.get(&Uuid::from_u128(1)).is_none());
        assert!(cache.get(&Uuid::from_u128(2)).is_some());
        assert_eq!(cache.bytes, 60);
    }

    #[test]
    fn archives_larger_than_the_budget_are_not_cached() {
        let mut cache = ArchiveCache::new(10);
        cache.insert(Uuid::from_u128(1), sized(50));
        assert!(cache.get(&Uuid::from_u128(1)).is_none());
        assert_eq!(cache.bytes, 0);
    }

    #[test]
    fn cache_evicts_the_least_recently_used_archive() {
        let mut cache = ArchiveCache::new(100);
        cache.insert(Uuid::from_u128(1), sized(40));
        cache.insert(Uuid::from_u128(2), sized(40));
        cache.get(&Uuid::from_u128(1));
        cache.insert(Uuid::from_u128(3), sized(40));
        assert!(cache.get(&Uuid::from_u128(1)).is_some());
        assert!(cache.get(&Uuid::from_u128(2)).is_none());
        assert!(cache.get(&Uuid::from_u128(3)).is_some());
    }

    #[test]
    fn host_cache_stays_below_its_cap_under_a_flood_of_hosts() {
        let mut cache = HostCache::default();
        for index in 0..(MAX_CACHED_HOSTS * 3) {
            cache.insert(format!("host-{index}.example"), None);
            assert!(cache.entries.len() <= MAX_CACHED_HOSTS);
        }
    }

    #[test]
    fn host_cache_drops_expired_entries_before_clearing() {
        let mut cache = HostCache::default();
        let expired = Instant::now() - HOST_TTL * 2;
        for index in 0..MAX_CACHED_HOSTS {
            cache
                .entries
                .insert(format!("old-{index}"), (None, expired));
        }
        cache
            .entries
            .insert("fresh".to_string(), (None, Instant::now()));
        cache.entries.remove("old-0");
        cache.insert("new".to_string(), None);
        assert!(cache.get("fresh").is_some());
        assert!(cache.get("new").is_some());
        assert_eq!(cache.entries.len(), 2);
    }

    #[test]
    fn replacing_an_entry_keeps_the_byte_count_exact() {
        let mut cache = ArchiveCache::new(100);
        cache.insert(Uuid::from_u128(1), sized(30));
        cache.insert(Uuid::from_u128(1), sized(40));
        assert_eq!(cache.bytes, 40);
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

    fn site_with(files: &[(&str, Vec<u8>)]) -> Archive {
        Archive::new(
            files
                .iter()
                .map(|(name, data)| (name.to_string(), Bytes::from(data.clone())))
                .collect(),
        )
    }

    fn request_with(pairs: &[(header::HeaderName, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(name.clone(), value.parse().unwrap());
        }
        headers
    }

    fn serve_path(site: &Archive, path: &str, request: &HeaderMap) -> Response {
        respond(site, locate(site, path).unwrap(), Uuid::nil(), request)
    }

    fn header_of(response: &Response, name: header::HeaderName) -> Option<&str> {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
    }

    fn script() -> Vec<u8> {
        "console.log(1);\n".repeat(100).into_bytes()
    }

    #[test]
    fn scripts_are_served_brotli_compressed_when_accepted() {
        let site = site_with(&[("app.js", script())]);
        let request = request_with(&[(header::ACCEPT_ENCODING, "gzip, br")]);
        let response = serve_path(&site, "/app.js", &request);
        assert_eq!(header_of(&response, header::CONTENT_ENCODING), Some("br"));
        assert_eq!(header_of(&response, header::VARY), Some("Accept-Encoding"));
    }

    #[test]
    fn clients_without_compression_get_the_original_bytes() {
        let site = site_with(&[("app.js", script())]);
        let response = serve_path(&site, "/app.js", &HeaderMap::new());
        assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
    }

    #[test]
    fn binary_files_are_never_compressed() {
        let site = site_with(&[("logo.png", vec![7; 4096])]);
        let request = request_with(&[(header::ACCEPT_ENCODING, "br, gzip")]);
        let response = serve_path(&site, "/logo.png", &request);
        assert!(response.headers().get(header::CONTENT_ENCODING).is_none());
        assert!(response.headers().get(header::VARY).is_none());
    }

    #[test]
    fn a_repeat_request_with_the_etag_gets_not_modified() {
        let site = site_with(&[("app.js", script())]);
        let first = serve_path(
            &site,
            "/app.js",
            &request_with(&[(header::ACCEPT_ENCODING, "br")]),
        );
        let etag = header_of(&first, header::ETAG).unwrap().to_string();
        let repeat = request_with(&[
            (header::ACCEPT_ENCODING, "br"),
            (header::IF_NONE_MATCH, &etag),
        ]);
        let second = serve_path(&site, "/app.js", &repeat);
        assert_eq!(second.status(), StatusCode::NOT_MODIFIED);
        assert_eq!(header_of(&second, header::ETAG), Some(etag.as_str()));
    }

    #[test]
    fn a_changed_file_does_not_match_an_old_etag() {
        let site = site_with(&[("app.js", script())]);
        let request = request_with(&[(header::IF_NONE_MATCH, "\"stale\"")]);
        assert_eq!(
            serve_path(&site, "/app.js", &request).status(),
            StatusCode::OK
        );
    }

    #[test]
    fn fingerprinted_assets_are_immutable_and_html_is_revalidated() {
        let site = site_with(&[
            ("_app/immutable/entry.js", script()),
            ("static.js", script()),
            ("index.html", b"<html></html>".to_vec()),
        ]);
        let none = HeaderMap::new();
        let cache = |path| {
            header_of(&serve_path(&site, path, &none), header::CACHE_CONTROL).map(str::to_string)
        };
        assert_eq!(
            cache("/_app/immutable/entry.js").as_deref(),
            Some(FINGERPRINTED_ASSET)
        );
        assert_eq!(cache("/static.js").as_deref(), Some(SHORT_LIVED_ASSET));
        assert_eq!(cache("/").as_deref(), Some(REVALIDATE));
    }

    #[test]
    fn error_pages_are_never_answered_with_not_modified() {
        let site = site_with(&[("404.html", b"<html>missing</html>".to_vec())]);
        let first = serve_path(&site, "/nope", &HeaderMap::new());
        let etag = header_of(&first, header::ETAG).unwrap().to_string();
        let repeat = request_with(&[(header::IF_NONE_MATCH, &etag)]);
        assert_eq!(
            serve_path(&site, "/nope", &repeat).status(),
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn host_header_is_normalized() {
        let mut headers = HeaderMap::new();
        headers.insert(header::HOST, "Shop.CSFX.dev:443".parse().unwrap());
        assert_eq!(host_of(&headers).as_deref(), Some("shop.csfx.dev"));
    }
}
