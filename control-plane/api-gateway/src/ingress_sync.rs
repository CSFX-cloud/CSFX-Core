use entity::entities::{agents, ingress_routes, workloads};
use etcd_client::{Client, GetOptions};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use serde_json::Value;
use shared::leader::LeaderLease;
use std::collections::{BTreeMap, HashMap};
use std::time::Duration;
use tokio::time::{interval, sleep};
use uuid::Uuid;

const LEADER_KEY: &str = "/csfx/leaders/ingress-sync";
const KEY_PREFIX: &str = "traefik/http/";
const OWNED_MARKER: &str = "/csfx-";
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

pub fn desired_entries(
    routes: &[RouteTarget],
    cert_resolver: Option<&str>,
) -> BTreeMap<String, String> {
    let mut entries: BTreeMap<String, String> = redirect_middleware_entries().into_iter().collect();
    for route in routes {
        let name = format!("csfx-{}", route.id);
        entries.extend(service_entries(&name, &route.backend));
        if route.tls {
            entries.extend(tls_router_entries(&name, &route.hostname, cert_resolver));
            entries.extend(redirect_router_entries(&name, &route.hostname));
        } else {
            entries.extend(plain_router_entries(&name, &route.hostname));
        }
    }
    entries
}

fn host_rule(hostname: &str) -> String {
    format!("Host(`{hostname}`)")
}

fn service_entries(name: &str, backend: &str) -> [(String, String); 1] {
    [(
        format!("{KEY_PREFIX}services/{name}/loadBalancer/servers/0/url"),
        backend.to_string(),
    )]
}

fn plain_router_entries(name: &str, hostname: &str) -> [(String, String); 3] {
    let base = format!("{KEY_PREFIX}routers/{name}");
    [
        (format!("{base}/rule"), host_rule(hostname)),
        (format!("{base}/service"), name.to_string()),
        (format!("{base}/entryPoints/0"), ENTRYPOINT_WEB.to_string()),
    ]
}

fn tls_router_entries(
    name: &str,
    hostname: &str,
    cert_resolver: Option<&str>,
) -> Vec<(String, String)> {
    let base = format!("{KEY_PREFIX}routers/{name}");
    let mut entries = vec![
        (format!("{base}/rule"), host_rule(hostname)),
        (format!("{base}/service"), name.to_string()),
        (
            format!("{base}/entryPoints/0"),
            ENTRYPOINT_WEBSECURE.to_string(),
        ),
        (format!("{base}/tls"), "true".to_string()),
    ];
    if let Some(resolver) = cert_resolver {
        entries.push((format!("{base}/tls/certResolver"), resolver.to_string()));
    }
    entries
}

fn redirect_router_entries(name: &str, hostname: &str) -> [(String, String); 4] {
    let base = format!("{KEY_PREFIX}routers/{name}-redirect");
    [
        (format!("{base}/rule"), host_rule(hostname)),
        (format!("{base}/service"), "noop@internal".to_string()),
        (format!("{base}/entryPoints/0"), ENTRYPOINT_WEB.to_string()),
        (
            format!("{base}/middlewares/0"),
            REDIRECT_MIDDLEWARE.to_string(),
        ),
    ]
}

fn redirect_middleware_entries() -> [(String, String); 2] {
    let base = format!("{KEY_PREFIX}middlewares/{REDIRECT_MIDDLEWARE}/redirectScheme");
    [
        (format!("{base}/scheme"), "https".to_string()),
        (format!("{base}/permanent"), "true".to_string()),
    ]
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

pub async fn load_targets(db: &DatabaseConnection) -> Result<Vec<RouteTarget>, DbErr> {
    let routes = ingress_routes::Entity::find().all(db).await?;
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

pub async fn reconcile_once(
    db: &DatabaseConnection,
    client: &mut Client,
    cert_resolver: Option<&str>,
) -> anyhow::Result<EntryDiff> {
    let targets = load_targets(db).await?;
    let desired = desired_entries(&targets, cert_resolver);
    let existing = existing_entries(client).await?;
    let diff = diff_entries(&existing, &desired);
    apply_diff(client, &diff).await?;
    Ok(diff)
}

pub fn spawn(db: DatabaseConnection) {
    tokio::spawn(run(db));
}

async fn run(db: DatabaseConnection) {
    let cert_resolver = std::env::var("INGRESS_CERT_RESOLVER")
        .ok()
        .filter(|resolver| !resolver.is_empty());
    let zone = crate::dns_sync::zone_from_env();
    let (leader, mut client) = connect_with_retry().await;
    let mut tick = interval(SYNC_INTERVAL);

    loop {
        tick.tick().await;
        if !leader.is_leader() {
            continue;
        }
        match reconcile_once(&db, &mut client, cert_resolver.as_deref()).await {
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
    use super::*;
    use serde_json::json;

    fn target(tls: bool) -> RouteTarget {
        RouteTarget {
            id: Uuid::nil(),
            hostname: "app.example.com".to_string(),
            tls,
            backend: "http://10.100.0.4:30001".to_string(),
        }
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
        let entries = desired_entries(&[target(false)], None);
        let router = format!("traefik/http/routers/csfx-{}", Uuid::nil());
        assert_eq!(
            entries[&format!("{router}/rule")],
            "Host(`app.example.com`)"
        );
        assert_eq!(entries[&format!("{router}/entryPoints/0")], "web");
        assert!(!entries.keys().any(|key| key.contains("-redirect/")));
        assert!(!entries.keys().any(|key| key.ends_with("/tls")));
    }

    #[test]
    fn the_subtree_is_never_empty() {
        let entries = desired_entries(&[], None);
        assert_eq!(
            entries["traefik/http/middlewares/csfx-redirect-https/redirectScheme/scheme"],
            "https"
        );
        assert_eq!(diff_entries(&entries, &entries), EntryDiff::default());
    }

    #[test]
    fn tls_route_adds_secure_router_redirect_and_resolver() {
        let entries = desired_entries(&[target(true)], Some("letsencrypt"));
        let router = format!("traefik/http/routers/csfx-{}", Uuid::nil());
        assert_eq!(entries[&format!("{router}/entryPoints/0")], "websecure");
        assert_eq!(entries[&format!("{router}/tls")], "true");
        assert_eq!(
            entries[&format!("{router}/tls/certResolver")],
            "letsencrypt"
        );
        assert_eq!(entries[&format!("{router}-redirect/entryPoints/0")], "web");
        assert_eq!(
            entries[&format!("{router}-redirect/middlewares/0")],
            "csfx-redirect-https"
        );
        assert_eq!(
            entries["traefik/http/middlewares/csfx-redirect-https/redirectScheme/scheme"],
            "https"
        );
    }

    #[test]
    fn tls_route_without_resolver_has_no_resolver_key() {
        let entries = desired_entries(&[target(true)], None);
        assert!(!entries.keys().any(|key| key.ends_with("certResolver")));
    }

    #[test]
    fn diff_puts_changes_and_deletes_only_owned_keys() {
        let mut existing = BTreeMap::new();
        existing.insert(
            "traefik/http/routers/csfx-a/rule".to_string(),
            "old".to_string(),
        );
        existing.insert(
            "traefik/http/routers/csfx-gone/rule".to_string(),
            "x".to_string(),
        );
        existing.insert(
            "traefik/http/routers/manual/rule".to_string(),
            "keep".to_string(),
        );
        existing.insert(
            "traefik/http/routers/csfx-same/rule".to_string(),
            "same".to_string(),
        );
        let mut desired = BTreeMap::new();
        desired.insert(
            "traefik/http/routers/csfx-a/rule".to_string(),
            "new".to_string(),
        );
        desired.insert(
            "traefik/http/routers/csfx-same/rule".to_string(),
            "same".to_string(),
        );
        desired.insert(
            "traefik/http/routers/csfx-b/rule".to_string(),
            "fresh".to_string(),
        );

        let diff = diff_entries(&existing, &desired);

        assert_eq!(
            diff.puts,
            vec![
                (
                    "traefik/http/routers/csfx-a/rule".to_string(),
                    "new".to_string()
                ),
                (
                    "traefik/http/routers/csfx-b/rule".to_string(),
                    "fresh".to_string()
                ),
            ]
        );
        assert_eq!(
            diff.deletes,
            vec!["traefik/http/routers/csfx-gone/rule".to_string()]
        );
    }

    #[tokio::test]
    #[ignore]
    async fn sync_roundtrip_against_real_etcd() {
        let endpoint = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS");
        let mut client = Client::connect([endpoint.as_str()], None).await.unwrap();
        let desired = desired_entries(&[target(true)], Some("letsencrypt"));

        let first = diff_entries(&existing_entries(&mut client).await.unwrap(), &desired);
        apply_diff(&mut client, &first).await.unwrap();
        let second = diff_entries(&existing_entries(&mut client).await.unwrap(), &desired);
        let cleanup = diff_entries(
            &existing_entries(&mut client).await.unwrap(),
            &BTreeMap::new(),
        );
        apply_diff(&mut client, &cleanup).await.unwrap();
        let after = diff_entries(
            &existing_entries(&mut client).await.unwrap(),
            &BTreeMap::new(),
        );

        assert!(!first.puts.is_empty());
        assert_eq!(second, EntryDiff::default());
        assert!(!cleanup.deletes.is_empty());
        assert!(after.deletes.is_empty());
    }
}
