use entity::entities::{agents, ingress_routes, site_domains, sites, workloads};
use etcd_client::{Client, GetOptions};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use serde_json::{json, Map, Value};
use shared::leader::LeaderLease;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tokio::time::{interval, sleep};
use uuid::Uuid;

const LEADER_KEY: &str = "/csfx/leaders/ingress-sync";
const KEY_PREFIX: &str = "traefik/http/";
const OWNED_MARKER: &str = "/csfx-";
const SITE_EDGE_VARIABLE: &str = "CSFX_SITE_EDGE_URL";
const REDIRECT_MIDDLEWARE: &str = "csfx-redirect-https";
const ENTRYPOINT_WEB: &str = "web";
const ENTRYPOINT_WEBSECURE: &str = "websecure";
const SYNC_INTERVAL: Duration = Duration::from_secs(10);
const CONNECT_RETRY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteTarget {
    pub id: Uuid,
    pub hostname: String,
    pub tls: bool,
    pub backend: String,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct EntryDiff {
    pub puts: Vec<(String, String)>,
    pub deletes: Vec<String>,
}

pub fn node_port_for(ports: &Value, container_port: u16) -> Option<u16> {
    ports.as_array()?.iter().find_map(|mapping| {
        let matches = mapping["container_port"].as_u64() == Some(u64::from(container_port));
        let node_port = mapping["node_port"].as_u64()?;
        if matches {
            u16::try_from(node_port).ok()
        } else {
            None
        }
    })
}

pub fn platform_entries() -> BTreeMap<String, String> {
    let base = format!("{KEY_PREFIX}middlewares/{REDIRECT_MIDDLEWARE}/redirectScheme");
    BTreeMap::from([
        (format!("{base}/scheme"), "https".to_string()),
        (format!("{base}/permanent"), "true".to_string()),
    ])
}

pub fn zone_hostnames(routes: &[RouteTarget], zone: &str) -> Vec<String> {
    let mut hostnames: Vec<String> = routes
        .iter()
        .filter(|route| route.tls && crate::dns_sync::in_zone(&route.hostname, zone))
        .map(|route| route.hostname.clone())
        .collect();
    hostnames.sort();
    hostnames.dedup();
    hostnames
}

fn host_rule(hostname: &str) -> String {
    format!("Host(`{hostname}`)")
}

pub fn provider_config(routes: &[RouteTarget], zone: &str, cert_resolver: Option<&str>) -> Value {
    let mut routers = Map::new();
    let mut services = Map::new();
    for route in routes {
        let name = format!("csfx-{}", route.id);
        services.insert(name.clone(), service_config(&route.backend));
        if !route.tls {
            routers.insert(name.clone(), plain_router(&name, &route.hostname));
            continue;
        }
        let resolver = cert_resolver.filter(|_| !crate::dns_sync::in_zone(&route.hostname, zone));
        routers.insert(name.clone(), tls_router(&name, &route.hostname, resolver));
        routers.insert(format!("{name}-redirect"), redirect_router(&route.hostname));
    }
    json!({
        "http": {
            "routers": routers,
            "services": services,
            "middlewares": {
                REDIRECT_MIDDLEWARE: {
                    "redirectScheme": { "scheme": "https", "permanent": true }
                }
            }
        }
    })
}

fn service_config(backend: &str) -> Value {
    json!({ "loadBalancer": { "servers": [{ "url": backend }] } })
}

fn plain_router(name: &str, hostname: &str) -> Value {
    json!({
        "rule": host_rule(hostname),
        "service": name,
        "entryPoints": [ENTRYPOINT_WEB]
    })
}

fn tls_router(name: &str, hostname: &str, cert_resolver: Option<&str>) -> Value {
    let tls = match cert_resolver {
        Some(resolver) => json!({ "certResolver": resolver }),
        None => json!({}),
    };
    json!({
        "rule": host_rule(hostname),
        "service": name,
        "entryPoints": [ENTRYPOINT_WEBSECURE],
        "tls": tls
    })
}

fn redirect_router(hostname: &str) -> Value {
    json!({
        "rule": host_rule(hostname),
        "service": "noop@internal",
        "entryPoints": [ENTRYPOINT_WEB],
        "middlewares": [REDIRECT_MIDDLEWARE]
    })
}

pub fn is_owned_key(key: &str) -> bool {
    key.starts_with(KEY_PREFIX) && key.contains(OWNED_MARKER)
}

pub fn diff_entries(
    existing: &BTreeMap<String, String>,
    desired: &BTreeMap<String, String>,
) -> EntryDiff {
    let puts = desired
        .iter()
        .filter(|(key, value)| existing.get(*key) != Some(*value))
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let deletes = existing
        .keys()
        .filter(|key| is_owned_key(key) && !desired.contains_key(*key))
        .cloned()
        .collect();
    EntryDiff { puts, deletes }
}

pub async fn load_targets(
    db: &DatabaseConnection,
    gateway_id: Uuid,
) -> Result<Vec<RouteTarget>, DbErr> {
    let mut targets = workload_targets(db, gateway_id).await?;
    targets.extend(site_targets(db, gateway_id).await?);
    Ok(targets)
}

pub fn domain_target(domain: site_domains::Model, backend: &str) -> RouteTarget {
    RouteTarget {
        id: domain.id,
        hostname: domain.hostname,
        tls: true,
        backend: backend.to_string(),
    }
}

pub fn site_edge_backend() -> Option<String> {
    std::env::var(SITE_EDGE_VARIABLE)
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn site_edge_address() -> Option<String> {
    let backend = site_edge_backend()?;
    let host = backend.split("://").last()?.split(['/', ':']).next()?;
    host.parse::<std::net::Ipv4Addr>()
        .ok()
        .map(|address| address.to_string())
}

pub fn site_edge_port() -> Option<u16> {
    let backend = site_edge_backend()?;
    let authority = backend.split("://").last()?.split('/').next()?;
    authority.rsplit_once(':')?.1.parse().ok()
}

async fn site_targets(
    db: &DatabaseConnection,
    gateway_id: Uuid,
) -> Result<Vec<RouteTarget>, DbErr> {
    let Some(backend) = site_edge_backend() else {
        return Ok(Vec::new());
    };
    let site_ids: Vec<Uuid> = sites::Entity::find()
        .filter(sites::Column::GatewayId.eq(gateway_id))
        .filter(sites::Column::Mode.eq("static"))
        .all(db)
        .await?
        .into_iter()
        .map(|site| site.id)
        .collect();
    if site_ids.is_empty() {
        return Ok(Vec::new());
    }
    Ok(site_domains::Entity::find()
        .filter(site_domains::Column::SiteId.is_in(site_ids))
        .filter(site_domains::Column::Kind.ne("internal"))
        .all(db)
        .await?
        .into_iter()
        .map(|domain| domain_target(domain, &backend))
        .collect())
}

async fn workload_targets(
    db: &DatabaseConnection,
    gateway_id: Uuid,
) -> Result<Vec<RouteTarget>, DbErr> {
    let routes = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::GatewayId.eq(gateway_id))
        .all(db)
        .await?;
    if routes.is_empty() {
        return Ok(Vec::new());
    }

    let workload_ids: Vec<Uuid> = routes.iter().map(|route| route.workload_id).collect();
    let workloads: HashMap<Uuid, workloads::Model> = workloads::Entity::find()
        .filter(workloads::Column::Id.is_in(workload_ids))
        .filter(workloads::Column::Status.eq("running"))
        .all(db)
        .await?
        .into_iter()
        .map(|workload| (workload.id, workload))
        .collect();

    let agent_ids: Vec<Uuid> = workloads
        .values()
        .filter_map(|workload| workload.assigned_agent_id)
        .collect();
    let agents: HashMap<Uuid, agents::Model> = agents::Entity::find()
        .filter(agents::Column::Id.is_in(agent_ids))
        .filter(agents::Column::Status.eq("Online"))
        .all(db)
        .await?
        .into_iter()
        .map(|agent| (agent.id, agent))
        .collect();

    Ok(routes
        .into_iter()
        .filter_map(|route| resolve_target(route, &workloads, &agents))
        .collect())
}

fn resolve_target(
    route: ingress_routes::Model,
    workloads: &HashMap<Uuid, workloads::Model>,
    agents: &HashMap<Uuid, agents::Model>,
) -> Option<RouteTarget> {
    let workload = workloads.get(&route.workload_id)?;
    let agent = agents.get(&workload.assigned_agent_id?)?;
    let tunnel_ip = agent.wg_tunnel_ip.as_deref()?;
    let container_port = u16::try_from(route.container_port).ok()?;
    let node_port = node_port_for(workload.ports.as_ref()?, container_port)?;

    Some(RouteTarget {
        id: route.id,
        hostname: route.hostname,
        tls: route.tls,
        backend: format!("http://{tunnel_ip}:{node_port}"),
    })
}

async fn existing_entries(
    client: &mut Client,
) -> Result<BTreeMap<String, String>, etcd_client::Error> {
    let response = client
        .get(KEY_PREFIX, Some(GetOptions::new().with_prefix()))
        .await?;
    Ok(response
        .kvs()
        .iter()
        .filter_map(|kv| {
            let key = kv.key_str().ok()?.to_string();
            let value = kv.value_str().ok()?.to_string();
            Some((key, value))
        })
        .collect())
}

pub async fn apply_diff(client: &mut Client, diff: &EntryDiff) -> Result<(), etcd_client::Error> {
    for (key, value) in &diff.puts {
        client.put(key.as_str(), value.as_str(), None).await?;
    }
    for key in &diff.deletes {
        client.delete(key.as_str(), None).await?;
    }
    Ok(())
}

pub async fn reconcile_once(client: &mut Client) -> anyhow::Result<EntryDiff> {
    let existing = existing_entries(client).await?;
    let diff = diff_entries(&existing, &platform_entries());
    apply_diff(client, &diff).await?;
    Ok(diff)
}

pub fn spawn(db: DatabaseConnection) {
    tokio::spawn(run(db));
}

async fn run(db: DatabaseConnection) {
    let zone = crate::dns_sync::zone_from_env();
    let (leader, mut client) = connect_with_retry().await;
    let mut tick = interval(SYNC_INTERVAL);

    loop {
        tick.tick().await;
        if !leader.is_leader() {
            continue;
        }
        match reconcile_once(&mut client).await {
            Ok(diff) if diff != EntryDiff::default() => {
                tracing::info!(
                    puts = diff.puts.len(),
                    deletes = diff.deletes.len(),
                    "ingress routes synchronized"
                );
            }
            Ok(_) => {}
            Err(e) => tracing::error!(error = %e, "ingress synchronization failed"),
        }
        sync_dns(&db, &mut client, &zone).await;
    }
}

async fn sync_dns(db: &DatabaseConnection, client: &mut Client, zone: &str) {
    match crate::dns_sync::reconcile_once(db, client, zone).await {
        Ok(diff) if diff != EntryDiff::default() => {
            tracing::info!(
                puts = diff.puts.len(),
                deletes = diff.deletes.len(),
                zone = %zone,
                "dns records synchronized"
            );
        }
        Ok(_) => {}
        Err(e) => tracing::error!(error = %e, zone = %zone, "dns synchronization failed"),
    }
}

async fn connect_with_retry() -> (LeaderLease, Client) {
    loop {
        match connect().await {
            Ok(pair) => return pair,
            Err(e) => {
                tracing::warn!(error = %e, "ingress synchronization waiting for etcd");
                sleep(CONNECT_RETRY).await;
            }
        }
    }
}

async fn connect() -> anyhow::Result<(LeaderLease, Client)> {
    let endpoints =
        std::env::var("ETCD_ENDPOINTS").unwrap_or_else(|_| "http://localhost:2379".into());
    let client = Client::connect(endpoints.split(',').collect::<Vec<_>>(), None).await?;
    let leader = LeaderLease::connect(LEADER_KEY).await?;
    Ok((leader, client))
}

#[cfg(test)]
mod tests {
    #[test]
    fn edge_address_is_the_ipv4_host_of_the_edge_url() {
        std::env::set_var("CSFX_SITE_EDGE_URL", "http://192.168.105.200:8010");
        assert_eq!(
            super::site_edge_address().as_deref(),
            Some("192.168.105.200")
        );
        assert_eq!(super::site_edge_port(), Some(8010));
        std::env::set_var("CSFX_SITE_EDGE_URL", "http://edge.internal:8010");
        assert_eq!(super::site_edge_address(), None);
        std::env::remove_var("CSFX_SITE_EDGE_URL");
    }

    use super::*;

    const ZONE: &str = "csfx.internal";

    fn target(hostname: &str, tls: bool) -> RouteTarget {
        RouteTarget {
            id: Uuid::nil(),
            hostname: hostname.to_string(),
            tls,
            backend: "http://10.100.0.4:30001".to_string(),
        }
    }

    fn router_name() -> String {
        format!("csfx-{}", Uuid::nil())
    }

    #[test]
    fn site_domains_route_to_the_edge_backend() {
        let domain = site_domains::Model {
            id: Uuid::from_u128(7),
            site_id: Uuid::nil(),
            hostname: "shop.csfx.dev".to_string(),
            kind: "production".to_string(),
            branch: None,
            created_at: chrono::NaiveDateTime::default(),
        };
        let target = domain_target(domain, "http://edge:8008");
        assert_eq!(target.hostname, "shop.csfx.dev");
        assert_eq!(target.backend, "http://edge:8008");
        assert!(target.tls);
    }

    #[test]
    fn node_port_is_looked_up_by_container_port() {
        let ports = json!([
            {"container_port": 80, "protocol": "tcp", "node_port": 30001},
            {"container_port": 5432, "node_port": null},
            {"container_port": 9000}
        ]);
        assert_eq!(node_port_for(&ports, 80), Some(30001));
        assert_eq!(node_port_for(&ports, 5432), None);
        assert_eq!(node_port_for(&ports, 9000), None);
        assert_eq!(node_port_for(&ports, 1), None);
        assert_eq!(node_port_for(&json!(null), 80), None);
    }

    #[test]
    fn plain_route_uses_the_web_entrypoint_only() {
        let config = provider_config(&[target("app.example.com", false)], ZONE, None);
        let routers = &config["http"]["routers"];
        let router = &routers[router_name()];
        assert_eq!(router["rule"], "Host(`app.example.com`)");
        assert_eq!(router["entryPoints"], json!(["web"]));
        assert!(router.get("tls").is_none());
        assert_eq!(routers.as_object().unwrap().len(), 1);
        assert_eq!(
            config["http"]["services"][router_name()]["loadBalancer"]["servers"][0]["url"],
            "http://10.100.0.4:30001"
        );
    }

    #[test]
    fn remote_tls_route_gets_resolver_and_redirect() {
        let config = provider_config(
            &[target("app.example.com", true)],
            ZONE,
            Some("letsencrypt"),
        );
        let routers = &config["http"]["routers"];
        let router = &routers[router_name()];
        assert_eq!(router["entryPoints"], json!(["websecure"]));
        assert_eq!(router["tls"]["certResolver"], "letsencrypt");
        let redirect = &routers[format!("{}-redirect", router_name())];
        assert_eq!(redirect["entryPoints"], json!(["web"]));
        assert_eq!(redirect["middlewares"], json!(["csfx-redirect-https"]));
        assert_eq!(
            config["http"]["middlewares"]["csfx-redirect-https"]["redirectScheme"]["permanent"],
            true
        );
    }

    #[test]
    fn zone_tls_route_never_uses_a_cert_resolver() {
        let config = provider_config(
            &[target("web.csfx.internal", true)],
            ZONE,
            Some("letsencrypt"),
        );
        let router = &config["http"]["routers"][router_name()];
        assert_eq!(router["tls"], json!({}));
    }

    #[test]
    fn certificate_names_cover_only_tls_routes_inside_the_zone() {
        let routes = [
            target("web.csfx.internal", true),
            target("web.csfx.internal", true),
            target("plain.csfx.internal", false),
            target("app.example.com", true),
        ];
        assert_eq!(zone_hostnames(&routes, ZONE), vec!["web.csfx.internal"]);
    }

    #[test]
    fn empty_gateway_still_serves_a_valid_document() {
        let config = provider_config(&[], ZONE, None);
        assert_eq!(config["http"]["routers"], json!({}));
        assert_eq!(config["http"]["services"], json!({}));
    }

    #[test]
    fn diff_puts_changes_and_deletes_only_owned_keys() {
        let mut existing = BTreeMap::new();
        existing.insert(
            "traefik/http/routers/csfx-gone/rule".to_string(),
            "x".to_string(),
        );
        existing.insert(
            "traefik/http/routers/manual/rule".to_string(),
            "keep".to_string(),
        );
        let diff = diff_entries(&existing, &platform_entries());

        assert_eq!(diff.puts.len(), 2);
        assert_eq!(
            diff.deletes,
            vec!["traefik/http/routers/csfx-gone/rule".to_string()]
        );
        assert_eq!(
            diff_entries(&platform_entries(), &platform_entries()),
            EntryDiff::default()
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sync_roundtrip_against_real_etcd() {
        let endpoint = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS");
        let mut client = Client::connect([endpoint.as_str()], None).await.unwrap();

        let first = reconcile_once(&mut client).await.unwrap();
        let second = reconcile_once(&mut client).await.unwrap();

        assert!(!first.puts.is_empty());
        assert_eq!(second, EntryDiff::default());
    }

    fn with_site_edge<T>(value: Option<&str>, check: impl FnOnce() -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved = std::env::var(SITE_EDGE_VARIABLE).ok();
        match value {
            Some(value) => std::env::set_var(SITE_EDGE_VARIABLE, value),
            None => std::env::remove_var(SITE_EDGE_VARIABLE),
        }
        let result = check();
        match saved {
            Some(saved) => std::env::set_var(SITE_EDGE_VARIABLE, saved),
            None => std::env::remove_var(SITE_EDGE_VARIABLE),
        }
        result
    }

    #[test]
    fn site_edge_is_absent_when_unset_or_empty() {
        for value in [None, Some("")] {
            with_site_edge(value, || {
                assert!(site_edge_backend().is_none());
                assert!(site_edge_address().is_none());
                assert!(site_edge_port().is_none());
            });
        }
    }

    #[test]
    fn site_edge_exposes_address_and_port_of_an_ip_backend() {
        with_site_edge(Some("http://10.1.2.3:8080/path"), || {
            assert_eq!(
                site_edge_backend().as_deref(),
                Some("http://10.1.2.3:8080/path")
            );
            assert_eq!(site_edge_address().as_deref(), Some("10.1.2.3"));
            assert_eq!(site_edge_port(), Some(8080));
        });
    }

    #[test]
    fn site_edge_hostnames_have_no_address_and_ports_are_optional() {
        with_site_edge(Some("http://edge.internal:9000"), || {
            assert!(site_edge_address().is_none());
            assert_eq!(site_edge_port(), Some(9000));
        });
        with_site_edge(Some("http://10.0.0.9"), || {
            assert_eq!(site_edge_address().as_deref(), Some("10.0.0.9"));
            assert!(site_edge_port().is_none());
        });
    }

    #[test]
    fn site_domains_become_tls_routes_to_the_edge() {
        let domain = site_domains::Model {
            id: Uuid::new_v4(),
            site_id: Uuid::new_v4(),
            hostname: "app.example.test".to_string(),
            kind: "custom".to_string(),
            branch: None,
            created_at: chrono::NaiveDateTime::default(),
        };
        let target = domain_target(domain.clone(), "http://10.0.0.9:80");
        assert_eq!(target.id, domain.id);
        assert_eq!(target.hostname, "app.example.test");
        assert!(target.tls);
        assert_eq!(target.backend, "http://10.0.0.9:80");
    }
}
