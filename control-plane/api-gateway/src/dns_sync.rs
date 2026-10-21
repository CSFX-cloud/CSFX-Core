use entity::entities::{
    agents, dns_records, ingress_gateways, ingress_routes, resource_groups, workloads,
};
use etcd_client::{Client, GetOptions};
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use std::collections::{BTreeMap, HashMap};
use uuid::Uuid;

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
    format!("{SKYDNS_ROOT}/{}/x1", reversed_labels(hostname))
}

fn record_value(target: &str, ttl: u32) -> String {
    serde_json::json!({ "host": target, "ttl": ttl }).to_string()
}

pub fn desired_records(
    zone: &str,
    gateways: &[GatewayAddress],
    routes: &[ingress_routes::Model],
    manual: &[dns_records::Model],
) -> BTreeMap<String, String> {
    let by_id: HashMap<Uuid, &GatewayAddress> = gateways
        .iter()
        .map(|gateway| (gateway.id, gateway))
        .collect();
    let mut records: BTreeMap<String, String> = manual
        .iter()
        .filter(|record| in_zone(&record.name, zone))
        .map(|record| {
            let ttl = u32::try_from(record.ttl).unwrap_or(RECORD_TTL);
            (record_key(&record.name), record_value(&record.value, ttl))
        })
        .collect();
    records.extend(
        gateways
            .iter()
            .filter(|gateway| in_zone(&gateway.endpoint, zone))
            .map(|gateway| {
                (
                    record_key(&gateway.endpoint),
                    record_value(&gateway.address, RECORD_TTL),
                )
            }),
    );
    for route in routes.iter().filter(|route| in_zone(&route.hostname, zone)) {
        let Some(gateway) = route.gateway_id.and_then(|id| by_id.get(&id)) else {
            continue;
        };
        records.insert(
            record_key(&route.hostname),
            record_value(&gateway.address, RECORD_TTL),
        );
    }
    records
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
            let workload = workloads.get(&gateway.workload_id)?;
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

async fn running_workloads(
    db: &DatabaseConnection,
    gateways: &[ingress_gateways::Model],
) -> Result<HashMap<Uuid, workloads::Model>, DbErr> {
    let ids: Vec<Uuid> = gateways.iter().map(|gateway| gateway.workload_id).collect();
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

pub async fn reconcile_once(
    db: &DatabaseConnection,
    client: &mut Client,
    zone: &str,
) -> anyhow::Result<EntryDiff> {
    let gateways = load_gateway_addresses(db, zone).await?;
    let routes = ingress_routes::Entity::find().all(db).await?;
    let manual = dns_records::Entity::find().all(db).await?;
    let desired = desired_records(zone, &gateways, &routes, &manual);
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

    fn gateway() -> GatewayAddress {
        GatewayAddress {
            id: Uuid::from_u128(1),
            endpoint: "gw.prod.csfx.internal".to_string(),
            address: "10.0.0.5".to_string(),
        }
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
        let records = desired_records("csfx.internal", &[gateway()], &routes, &[]);
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
            &[
                manual("db.csfx.internal", "10.0.0.9"),
                manual("gw.prod.csfx.internal", "10.9.9.9"),
                manual("elsewhere.example.com", "10.0.0.1"),
            ],
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
}
