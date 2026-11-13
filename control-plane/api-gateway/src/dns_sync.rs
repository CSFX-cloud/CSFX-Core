use entity::entities::{
    agents, dns_records, ingress_gateways, ingress_routes, resource_groups, site_domains, sites,
    workloads,
};
use etcd_client::{Client, GetOptions};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

use crate::hostname_policy::is_platform_name;
use crate::ingress_sync::{apply_diff, EntryDiff};

const SKYDNS_ROOT: &str = "/skydns";
const RECORD_TTL: u32 = 30;
const DEFAULT_ZONE: &str = "csfx.internal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayAddress {
    pub id: Uuid,
    pub endpoint: String,
    pub address: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlatformRecord {
    pub hostname: String,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SiteHost {
    pub hostname: String,
    pub gateway_id: Option<Uuid>,
    pub edge_address: Option<String>,
}

pub fn platform_hostname(zone: &str) -> String {
    std::env::var("CSFX_PLATFORM_HOSTNAME")
        .ok()
        .map(|name| name.trim().trim_matches('.').to_ascii_lowercase())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| format!("console.{zone}"))
}

pub fn zone_from_env() -> String {
    std::env::var("CSFX_LOCAL_ZONE")
        .ok()
        .map(|zone| zone.trim().trim_matches('.').to_ascii_lowercase())
        .filter(|zone| !zone.is_empty())
        .unwrap_or_else(|| DEFAULT_ZONE.to_string())
}

pub fn servers_from_env() -> Vec<String> {
    std::env::var("CSFX_LOCAL_DNS_ADDRESSES")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|address| !address.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn in_zone(hostname: &str, zone: &str) -> bool {
    hostname
        .strip_suffix(zone)
        .is_some_and(|prefix| prefix.ends_with('.'))
}

pub fn zone_prefix(zone: &str) -> String {
    format!("{SKYDNS_ROOT}/{}/", reversed_labels(zone))
}

fn reversed_labels(name: &str) -> String {
    let mut labels: Vec<&str> = name.split('.').collect();
    labels.reverse();
    labels.join("/")
}

pub fn record_key(hostname: &str) -> String {
    record_key_at(hostname, 1)
}

fn record_key_at(hostname: &str, position: usize) -> String {
    format!("{SKYDNS_ROOT}/{}/x{position}", reversed_labels(hostname))
}

fn record_value(target: &str, ttl: u32) -> String {
    serde_json::json!({ "host": target, "ttl": ttl }).to_string()
}

pub fn desired_records(
    zone: &str,
    gateways: &[GatewayAddress],
    routes: &[ingress_routes::Model],
    site_hosts: &[SiteHost],
    manual: &[dns_records::Model],
    platform: &PlatformRecord,
) -> BTreeMap<String, String> {
    let by_id: HashMap<Uuid, &GatewayAddress> = gateways
        .iter()
        .map(|gateway| (gateway.id, gateway))
        .collect();
    let publishable =
        |hostname: &str| in_zone(hostname, zone) && !is_platform_name(hostname, &platform.hostname);
    let mut records: BTreeMap<String, String> = manual
        .iter()
        .filter(|record| publishable(&record.name))
        .map(|record| {
            let ttl = u32::try_from(record.ttl).unwrap_or(RECORD_TTL);
            (record_key(&record.name), record_value(&record.value, ttl))
        })
        .collect();
    records.extend(
        gateways
            .iter()
            .filter(|gateway| publishable(&gateway.endpoint))
            .map(|gateway| {
                (
                    record_key(&gateway.endpoint),
                    record_value(&gateway.address, RECORD_TTL),
                )
            }),
    );
    for route in routes.iter().filter(|route| publishable(&route.hostname)) {
        let Some(gateway) = route.gateway_id.and_then(|id| by_id.get(&id)) else {
            continue;
        };
        records.insert(
            record_key(&route.hostname),
            record_value(&gateway.address, RECORD_TTL),
        );
    }
    for host in site_hosts.iter().filter(|host| publishable(&host.hostname)) {
        let Some(address) = site_host_address(host, &by_id) else {
            continue;
        };
        records.insert(
            record_key(&host.hostname),
            record_value(address, RECORD_TTL),
        );
    }
    if in_zone(&platform.hostname, zone) {
        records.extend(platform_records(platform));
    }
    records
}

fn site_host_address<'a>(
    host: &'a SiteHost,
    gateways: &HashMap<Uuid, &'a GatewayAddress>,
) -> Option<&'a str> {
    match host.gateway_id {
        Some(id) => gateways.get(&id).map(|gateway| gateway.address.as_str()),
        None => host.edge_address.as_deref(),
    }
}

fn platform_records(platform: &PlatformRecord) -> Vec<(String, String)> {
    let mut addresses: Vec<&String> = platform.addresses.iter().collect();
    addresses.sort();
    addresses.dedup();
    addresses
        .into_iter()
        .enumerate()
        .map(|(index, address)| {
            (
                record_key_at(&platform.hostname, index + 1),
                record_value(address, RECORD_TTL),
            )
        })
        .collect()
}

pub fn diff_records(
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
        .filter(|key| !desired.contains_key(*key))
        .cloned()
        .collect();
    EntryDiff { puts, deletes }
}

pub async fn load_gateway_addresses(
    db: &DatabaseConnection,
    zone: &str,
) -> Result<Vec<GatewayAddress>, DbErr> {
    let gateways = ingress_gateways::Entity::find().all(db).await?;
    if gateways.is_empty() {
        return Ok(Vec::new());
    }
    let workloads = running_workloads(db, &gateways).await?;
    let agents = online_agents(db, &workloads).await?;
    let groups = group_names(db, &gateways).await?;

    Ok(gateways
        .into_iter()
        .filter_map(|gateway| {
            let workload = workloads.get(&gateway.workload_id?)?;
            let agent = agents.get(&workload.assigned_agent_id?)?;
            let group = groups.get(&gateway.resource_group_id)?;
            Some(GatewayAddress {
                id: gateway.id,
                endpoint: format!("{}.{group}.{zone}", gateway.name),
                address: agent.ip_address.clone()?,
            })
        })
        .collect())
}

pub async fn load_platform_addresses(db: &DatabaseConnection) -> Result<Vec<String>, DbErr> {
    let configured = std::env::var("CSFX_PLATFORM_ADDRESSES").unwrap_or_default();
    let addresses: Vec<String> = configured
        .split(',')
        .map(str::trim)
        .filter(|address| !address.is_empty())
        .map(str::to_string)
        .collect();
    if !addresses.is_empty() {
        return Ok(addresses);
    }
    Ok(agents::Entity::find()
        .filter(agents::Column::Role.eq(crate::routes::masters::ROLE_MASTER))
        .filter(agents::Column::Status.eq("Online"))
        .all(db)
        .await?
        .into_iter()
        .filter_map(|agent| agent.ip_address)
        .collect())
}

async fn running_workloads(
    db: &DatabaseConnection,
    gateways: &[ingress_gateways::Model],
) -> Result<HashMap<Uuid, workloads::Model>, DbErr> {
    let ids: Vec<Uuid> = gateways
        .iter()
        .filter_map(|gateway| gateway.workload_id)
        .collect();
    Ok(workloads::Entity::find()
        .filter(workloads::Column::Id.is_in(ids))
        .filter(workloads::Column::Status.eq("running"))
        .all(db)
        .await?
        .into_iter()
        .map(|workload| (workload.id, workload))
        .collect())
}

async fn online_agents(
    db: &DatabaseConnection,
    workloads: &HashMap<Uuid, workloads::Model>,
) -> Result<HashMap<Uuid, agents::Model>, DbErr> {
    let ids: Vec<Uuid> = workloads
        .values()
        .filter_map(|workload| workload.assigned_agent_id)
        .collect();
    Ok(agents::Entity::find()
        .filter(agents::Column::Id.is_in(ids))
        .filter(agents::Column::Status.eq("Online"))
        .all(db)
        .await?
        .into_iter()
        .map(|agent| (agent.id, agent))
        .collect())
}

async fn group_names(
    db: &DatabaseConnection,
    gateways: &[ingress_gateways::Model],
) -> Result<HashMap<Uuid, String>, DbErr> {
    let ids: Vec<Uuid> = gateways
        .iter()
        .map(|gateway| gateway.resource_group_id)
        .collect();
    Ok(resource_groups::Entity::find()
        .filter(resource_groups::Column::Id.is_in(ids))
        .all(db)
        .await?
        .into_iter()
        .map(|group| (group.id, group.name.to_ascii_lowercase()))
        .collect())
}

async fn existing_records(
    client: &mut Client,
    zone: &str,
) -> Result<BTreeMap<String, String>, etcd_client::Error> {
    let response = client
        .get(zone_prefix(zone), Some(GetOptions::new().with_prefix()))
        .await?;
    Ok(response
        .kvs()
        .iter()
        .filter_map(|kv| {
            Some((
                kv.key_str().ok()?.to_string(),
                kv.value_str().ok()?.to_string(),
            ))
        })
        .collect())
}

async fn load_site_hosts(db: &DatabaseConnection) -> Result<Vec<SiteHost>, DbErr> {
    if crate::ingress_sync::site_edge_backend().is_none() {
        return Ok(Vec::new());
    }
    let gateways: HashMap<Uuid, Option<Uuid>> = sites::Entity::find()
        .filter(sites::Column::Mode.eq("static"))
        .all(db)
        .await?
        .into_iter()
        .map(|site| (site.id, site.gateway_id))
        .collect();
    let edge_address = crate::ingress_sync::site_edge_address();
    Ok(site_domains::Entity::find()
        .filter(site_domains::Column::SiteId.is_in(gateways.keys().copied().collect::<Vec<_>>()))
        .all(db)
        .await?
        .into_iter()
        .filter(|domain| domain.kind != "internal")
        .filter_map(|domain| {
            Some(SiteHost {
                gateway_id: *gateways.get(&domain.site_id)?,
                edge_address: edge_address.clone(),
                hostname: domain.hostname,
            })
        })
        .collect())
}

pub async fn reconcile_once(
    db: &DatabaseConnection,
    client: &mut Client,
    zone: &str,
) -> anyhow::Result<EntryDiff> {
    let gateways = load_gateway_addresses(db, zone).await?;
    let routes = ingress_routes::Entity::find().all(db).await?;
    let site_hosts = load_site_hosts(db).await?;
    let manual = dns_records::Entity::find().all(db).await?;
    let platform = PlatformRecord {
        hostname: platform_hostname(zone),
        addresses: load_platform_addresses(db).await?,
    };
    let desired = desired_records(zone, &gateways, &routes, &site_hosts, &manual, &platform);
    let diff = diff_records(&existing_records(client, zone).await?, &desired);
    apply_diff(client, &diff).await?;
    Ok(diff)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    fn route(hostname: &str, gateway_id: Option<Uuid>) -> ingress_routes::Model {
        ingress_routes::Model {
            id: Uuid::nil(),
            resource_group_id: Uuid::nil(),
            workload_id: Uuid::nil(),
            gateway_id,
            hostname: hostname.to_string(),
            container_port: 80,
            tls: true,
            created_at: NaiveDateTime::default(),
        }
    }

    fn no_platform() -> PlatformRecord {
        PlatformRecord {
            hostname: "console.csfx.internal".to_string(),
            addresses: Vec::new(),
        }
    }

    fn gateway() -> GatewayAddress {
        GatewayAddress {
            id: Uuid::from_u128(1),
            endpoint: "gw.prod.csfx.internal".to_string(),
            address: "10.0.0.5".to_string(),
        }
    }

    #[test]
    fn names_below_the_platform_hostname_are_never_published() {
        let manual = dns_records::Model {
            id: Uuid::nil(),
            resource_group_id: Uuid::nil(),
            name: "evil.console.csfx.internal".to_string(),
            record_type: "A".to_string(),
            value: "6.6.6.6".to_string(),
            ttl: 30,
            created_at: NaiveDateTime::default(),
        };
        let routes = [route("login.console.csfx.internal", Some(gateway().id))];
        let records = desired_records(
            "csfx.internal",
            &[gateway()],
            &routes,
            &[],
            &[manual],
            &no_platform(),
        );
        assert!(records.keys().all(|key| !key.contains("/console/")));
    }

    #[test]
    fn static_site_domains_resolve_to_their_gateway() {
        let hosts = [
            SiteHost {
                hostname: "shop.csfx.internal".to_string(),
                gateway_id: Some(Uuid::from_u128(1)),
                edge_address: None,
            },
            SiteHost {
                hostname: "shop.example.com".to_string(),
                gateway_id: Some(Uuid::from_u128(1)),
                edge_address: None,
            },
            SiteHost {
                hostname: "orphan.csfx.internal".to_string(),
                gateway_id: Some(Uuid::from_u128(9)),
                edge_address: None,
            },
            SiteHost {
                hostname: "direct.csfx.internal".to_string(),
                gateway_id: None,
                edge_address: Some("10.0.0.9".to_string()),
            },
        ];
        let records = desired_records(
            "csfx.internal",
            &[gateway()],
            &[],
            &hosts,
            &[],
            &no_platform(),
        );
        assert_eq!(
            records["/skydns/internal/csfx/shop/x1"],
            r#"{"host":"10.0.0.5","ttl":30}"#
        );
        assert!(!records.contains_key("/skydns/internal/csfx/orphan/x1"));
        assert!(!records.keys().any(|key| key.contains("example")));
        assert_eq!(
            records["/skydns/internal/csfx/direct/x1"],
            r#"{"host":"10.0.0.9","ttl":30}"#
        );
    }

    #[test]
    fn keys_reverse_labels_under_skydns_root() {
        assert_eq!(
            record_key("web.csfx.internal"),
            "/skydns/internal/csfx/web/x1"
        );
        assert_eq!(zone_prefix("csfx.internal"), "/skydns/internal/csfx/");
    }

    #[test]
    fn zone_membership_requires_a_label_boundary() {
        assert!(in_zone("web.csfx.internal", "csfx.internal"));
        assert!(!in_zone("csfx.internal", "csfx.internal"));
        assert!(!in_zone("webcsfx.internal", "csfx.internal"));
        assert!(!in_zone("web.example.com", "csfx.internal"));
    }

    #[test]
    fn records_cover_endpoint_and_in_zone_routes_only() {
        let routes = [
            route("web.csfx.internal", Some(Uuid::from_u128(1))),
            route("public.example.com", Some(Uuid::from_u128(1))),
            route("orphan.csfx.internal", None),
            route("lost.csfx.internal", Some(Uuid::from_u128(9))),
        ];
        let records = desired_records(
            "csfx.internal",
            &[gateway()],
            &routes,
            &[],
            &[],
            &no_platform(),
        );
        let keys: Vec<&str> = records.keys().map(String::as_str).collect();
        assert_eq!(
            keys,
            vec![
                "/skydns/internal/csfx/prod/gw/x1",
                "/skydns/internal/csfx/web/x1"
            ]
        );
        assert_eq!(
            records["/skydns/internal/csfx/web/x1"],
            r#"{"host":"10.0.0.5","ttl":30}"#
        );
    }

    #[test]
    fn manual_records_are_published_and_never_override_gateway_names() {
        let manual = |name: &str, value: &str| dns_records::Model {
            id: Uuid::nil(),
            resource_group_id: Uuid::nil(),
            name: name.to_string(),
            record_type: "A".to_string(),
            value: value.to_string(),
            ttl: 60,
            created_at: NaiveDateTime::default(),
        };
        let records = desired_records(
            "csfx.internal",
            &[gateway()],
            &[],
            &[],
            &[
                manual("db.csfx.internal", "10.0.0.9"),
                manual("gw.prod.csfx.internal", "10.9.9.9"),
                manual("elsewhere.example.com", "10.0.0.1"),
            ],
            &no_platform(),
        );
        assert_eq!(
            records["/skydns/internal/csfx/db/x1"],
            r#"{"host":"10.0.0.9","ttl":60}"#
        );
        assert_eq!(
            records["/skydns/internal/csfx/prod/gw/x1"],
            r#"{"host":"10.0.0.5","ttl":30}"#
        );
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn platform_hostname_gets_one_record_per_distinct_address() {
        let platform = PlatformRecord {
            hostname: "console.csfx.internal".to_string(),
            addresses: vec![
                "10.0.0.2".to_string(),
                "10.0.0.1".to_string(),
                "10.0.0.1".to_string(),
            ],
        };
        let records = desired_records("csfx.internal", &[], &[], &[], &[], &platform);
        assert_eq!(
            records["/skydns/internal/csfx/console/x1"],
            r#"{"host":"10.0.0.1","ttl":30}"#
        );
        assert_eq!(
            records["/skydns/internal/csfx/console/x2"],
            r#"{"host":"10.0.0.2","ttl":30}"#
        );
        assert_eq!(records.len(), 2);
    }

    #[test]
    fn platform_outside_the_zone_is_not_published() {
        let platform = PlatformRecord {
            hostname: "console.example.com".to_string(),
            addresses: vec!["10.0.0.1".to_string()],
        };
        assert!(desired_records("csfx.internal", &[], &[], &[], &[], &platform).is_empty());
    }

    #[test]
    fn diff_removes_every_stale_key_under_the_zone() {
        let existing = BTreeMap::from([
            ("/skydns/internal/csfx/old/x1".to_string(), "a".to_string()),
            ("/skydns/internal/csfx/same/x1".to_string(), "b".to_string()),
        ]);
        let desired = BTreeMap::from([
            ("/skydns/internal/csfx/same/x1".to_string(), "b".to_string()),
            ("/skydns/internal/csfx/new/x1".to_string(), "c".to_string()),
        ]);
        let diff = diff_records(&existing, &desired);
        assert_eq!(diff.deletes, vec!["/skydns/internal/csfx/old/x1"]);
        assert_eq!(
            diff.puts,
            vec![("/skydns/internal/csfx/new/x1".to_string(), "c".to_string())]
        );
    }

    fn with_variables<T>(pairs: &[(&str, Option<&str>)], check: impl FnOnce() -> T) -> T {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let saved: Vec<_> = pairs
            .iter()
            .map(|(name, _)| (*name, std::env::var(name).ok()))
            .collect();
        for (name, value) in pairs {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        let result = check();
        for (name, value) in saved {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
        result
    }

    #[test]
    fn zone_defaults_and_normalizes_environment_values() {
        with_variables(&[("CSFX_LOCAL_ZONE", None)], || {
            assert_eq!(zone_from_env(), "csfx.internal");
        });
        with_variables(&[("CSFX_LOCAL_ZONE", Some("  .Example.Test. "))], || {
            assert_eq!(zone_from_env(), "example.test");
        });
        with_variables(&[("CSFX_LOCAL_ZONE", Some("  "))], || {
            assert_eq!(zone_from_env(), "csfx.internal");
        });
    }

    #[test]
    fn platform_hostname_defaults_to_the_console_of_the_zone() {
        with_variables(&[("CSFX_PLATFORM_HOSTNAME", None)], || {
            assert_eq!(platform_hostname("csfx.internal"), "console.csfx.internal");
        });
        with_variables(
            &[("CSFX_PLATFORM_HOSTNAME", Some(" .Cloud.Example. "))],
            || {
                assert_eq!(platform_hostname("csfx.internal"), "cloud.example");
            },
        );
        with_variables(&[("CSFX_PLATFORM_HOSTNAME", Some(""))], || {
            assert_eq!(platform_hostname("z.test"), "console.z.test");
        });
    }

    #[test]
    fn dns_servers_are_split_trimmed_and_filtered() {
        with_variables(&[("CSFX_LOCAL_DNS_ADDRESSES", None)], || {
            assert!(servers_from_env().is_empty());
        });
        with_variables(
            &[("CSFX_LOCAL_DNS_ADDRESSES", Some(" 10.0.0.1, ,10.0.0.2 ,"))],
            || assert_eq!(servers_from_env(), vec!["10.0.0.1", "10.0.0.2"]),
        );
    }

    #[test]
    fn hostnames_belong_to_a_zone_only_below_a_dot_boundary() {
        assert!(in_zone("a.csfx.internal", "csfx.internal"));
        assert!(!in_zone("csfx.internal", "csfx.internal"));
        assert!(!in_zone("acsfx.internal", "csfx.internal"));
        assert!(!in_zone("a.other.test", "csfx.internal"));
    }

    #[test]
    fn record_keys_reverse_the_label_order() {
        assert_eq!(record_key("a.b.example"), "/skydns/example/b/a/x1");
        assert_eq!(zone_prefix("csfx.internal"), "/skydns/internal/csfx/");
    }
}
