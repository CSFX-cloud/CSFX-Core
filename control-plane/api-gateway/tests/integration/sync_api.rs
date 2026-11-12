use std::time::Duration;

use api_gateway::routes::agent_stream::AgentStreamRegistry;
use api_gateway::{
    dns_sync, etcd_members, ingress_sync, prune, service_client, zone_cert, AppState,
};
use chrono::{NaiveDate, Utc};
use entity::entities::{dns_records, ingress_gateways, logs};
use etcd_client::{Client, GetOptions};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use sea_orm::{ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_a::{insert_agent, insert_group, insert_workload, unique, WorkloadSpec};

async fn etcd() -> Client {
    let endpoints = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS must be set");
    Client::connect(endpoints.split(',').collect::<Vec<_>>(), None)
        .await
        .expect("etcd must be reachable")
}

fn unique_zone() -> String {
    format!("z{}.test", Uuid::new_v4().simple())
}

async fn zone_values(client: &mut Client, zone: &str) -> Vec<(String, String)> {
    let response = client
        .get(
            dns_sync::zone_prefix(zone),
            Some(GetOptions::new().with_prefix()),
        )
        .await
        .unwrap();
    response
        .kvs()
        .iter()
        .map(|kv| {
            (
                kv.key_str().unwrap().to_string(),
                kv.value_str().unwrap().to_string(),
            )
        })
        .collect()
}

async fn drop_zone(client: &mut Client, zone: &str) {
    let options = etcd_client::DeleteOptions::new().with_prefix();
    client
        .delete(dns_sync::zone_prefix(zone), Some(options))
        .await
        .unwrap();
}

async fn manual_record(harness: &Harness, group_id: Uuid, name: &str) -> dns_records::Model {
    dns_records::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        name: Set(name.to_string()),
        record_type: Set("A".to_string()),
        value: Set("10.20.30.40".to_string()),
        ttl: Set(60),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&harness.db)
    .await
    .unwrap()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn dns_reconciliation_publishes_and_prunes_records() {
    let harness = harness().await;
    let mut client = etcd().await;
    let zone = unique_zone();
    let group = insert_group(&harness.db, harness.org_id).await;
    let name = format!("rec.{zone}");
    let record = manual_record(&harness, group.id, &name).await;

    let diff = dns_sync::reconcile_once(&harness.db, &mut client, &zone)
        .await
        .unwrap();
    let key = dns_sync::record_key(&name);
    assert!(diff
        .puts
        .iter()
        .any(|(put, value)| *put == key && value.contains("10.20.30.40")));
    assert!(zone_values(&mut client, &zone)
        .await
        .iter()
        .any(|(stored, _)| *stored == key));
    let again = dns_sync::reconcile_once(&harness.db, &mut client, &zone)
        .await
        .unwrap();
    assert!(again.puts.iter().all(|(put, _)| *put != key));
    assert!(!again.deletes.contains(&key));

    dns_records::Entity::delete_by_id(record.id)
        .exec(&harness.db)
        .await
        .unwrap();
    let pruned = dns_sync::reconcile_once(&harness.db, &mut client, &zone)
        .await
        .unwrap();
    assert!(pruned.deletes.contains(&key));
    drop_zone(&mut client, &zone).await;
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn dns_reconciliation_resolves_gateway_endpoints() {
    let harness = harness().await;
    let mut client = etcd().await;
    let zone = unique_zone();
    let group = insert_group(&harness.db, harness.org_id).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let workload = insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "running"),
    )
    .await;
    let gateway = ingress_gateways::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group.id),
        workload_id: Set(workload.id),
        name: Set("edge".to_string()),
        http_port: Set(80),
        https_port: Set(443),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&harness.db)
    .await
    .unwrap();

    let addresses = dns_sync::load_gateway_addresses(&harness.db, &zone)
        .await
        .unwrap();
    let resolved = addresses
        .iter()
        .find(|address| address.id == gateway.id)
        .unwrap();
    assert_eq!(
        resolved.endpoint,
        format!("edge.{}.{zone}", group.name.to_ascii_lowercase())
    );
    assert_eq!(Some(&resolved.address), agent.ip_address.as_ref());

    dns_sync::reconcile_once(&harness.db, &mut client, &zone)
        .await
        .unwrap();
    let stored = zone_values(&mut client, &zone).await;
    assert!(stored
        .iter()
        .any(|(key, _)| *key == dns_sync::record_key(&resolved.endpoint)));
    drop_zone(&mut client, &zone).await;
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn platform_addresses_come_from_online_masters() {
    let harness = harness().await;
    let master = insert_agent(&harness.db, "master", "Online").await;
    let offline = insert_agent(&harness.db, "master", "Offline").await;
    let addresses = dns_sync::load_platform_addresses(&harness.db)
        .await
        .unwrap();
    assert!(addresses.contains(master.ip_address.as_ref().unwrap()));
    assert!(!addresses.contains(offline.ip_address.as_ref().unwrap()));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ingress_reconciliation_owns_only_its_keys() {
    let mut client = etcd().await;
    let stale = format!("traefik/http/routers/csfx-{}/rule", Uuid::new_v4());
    let foreign = format!("traefik/http/routers/{}/rule", unique("foreign"));
    client.put(stale.as_str(), "x", None).await.unwrap();
    client.put(foreign.as_str(), "x", None).await.unwrap();

    ingress_sync::reconcile_once(&mut client).await.unwrap();
    assert!(client
        .get(stale.as_str(), None)
        .await
        .unwrap()
        .kvs()
        .is_empty());
    assert_eq!(
        client
            .get(foreign.as_str(), None)
            .await
            .unwrap()
            .kvs()
            .len(),
        1
    );
    let settled = ingress_sync::reconcile_once(&mut client).await.unwrap();
    assert!(settled.puts.is_empty() && settled.deletes.is_empty());
    client.delete(foreign.as_str(), None).await.unwrap();
}

fn certificate_authority() -> (String, String) {
    let key = KeyPair::generate().unwrap();
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let cert = params.self_signed(&key).unwrap();
    (cert.pem(), key.serialize_pem())
}

#[test]
fn zone_certificates_are_issued_and_renewed_on_demand() {
    let (ca_cert, ca_key) = certificate_authority();
    let today = NaiveDate::from_ymd_opt(2026, 11, 6).unwrap();
    let sans = vec!["app.csfx.internal".to_string()];
    let scope = zone_cert::NameScope {
        zone: "csfx.internal",
        excluded: "console.csfx.internal",
    };

    assert!(zone_cert::needs_issue(None, &sans, today));
    let issued = zone_cert::issue(&sans, &scope, &ca_cert, &ca_key, today).unwrap();
    assert!(issued.cert_pem.contains("BEGIN CERTIFICATE"));
    assert!(!zone_cert::needs_issue(Some(&issued), &sans, today));
    assert!(zone_cert::needs_issue(
        Some(&issued),
        &sans,
        today + chrono::Duration::days(70)
    ));
    assert!(zone_cert::needs_issue(
        Some(&issued),
        &["other.csfx.internal".to_string()],
        today
    ));
    let section = zone_cert::tls_section(&issued);
    assert_eq!(
        section["certificates"][0]["certFile"],
        json!(issued.cert_pem)
    );
    assert!(zone_cert::issue(
        &["b\u{e4}d.example".to_string()],
        &scope,
        &ca_cert,
        &ca_key,
        today
    )
    .is_err());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn certificate_lookup_skips_empty_san_lists() {
    let harness = harness().await;
    let gateway_id = Uuid::new_v4();
    assert!(zone_cert::certificate_for(&harness.db, gateway_id, &[])
        .await
        .unwrap()
        .is_none());
    zone_cert::remove(&harness.db, gateway_id).await.unwrap();
}

#[test]
fn system_collector_reports_host_metrics() {
    let collector = api_gateway::system_collector::LocalSystemCollector::new();
    let metrics = collector.collect();
    assert!(metrics.memory_total_bytes > 0);
    assert!(metrics.cpu_cores > 0);
    assert!(!metrics.hostname.is_empty());
    assert!((0.0..=100.0).contains(&metrics.memory_usage_percent));
}

fn state_for(harness: &Harness) -> AppState {
    AppState {
        db_conn: harness.db.clone(),
        service_client: service_client::ServiceClient::new(),
        default_org_id: Some(harness.org_id),
        agent_stream_registry: AgentStreamRegistry::new(),
        openbao: None,
        site_builder: None,
    }
}

async fn log_aged(harness: &Harness, days: i64) -> Uuid {
    let at = (Utc::now() - chrono::Duration::days(days)).fixed_offset();
    let id = Uuid::new_v4();
    logs::ActiveModel {
        id: Set(id),
        service: Set(unique("prune")),
        level: Set("INFO".to_string()),
        classification: Set("system".to_string()),
        message: Set("aged".to_string()),
        agent_id: Set(None),
        workload_id: Set(None),
        organization_id: Set(None),
        created_at: Set(at),
        kind: Set("system".to_string()),
        actor_user_id: Set(None),
        action: Set(None),
        target_type: Set(None),
        target_id: Set(None),
        fields: Set(json!({})),
        occurrences: Set(1),
        last_seen_at: Set(at),
    }
    .insert(&harness.db)
    .await
    .unwrap();
    id
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn prune_job_removes_logs_past_retention() {
    let harness = harness().await;
    let expired = log_aged(&harness, 3650).await;
    let fresh = log_aged(&harness, 0).await;
    prune::spawn_log_prune_job(state_for(&harness));

    for _ in 0..50 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        let remaining = logs::Entity::find()
            .filter(logs::Column::Id.eq(expired))
            .one(&harness.db)
            .await
            .unwrap();
        if remaining.is_none() {
            break;
        }
    }
    let gone = logs::Entity::find_by_id(expired)
        .one(&harness.db)
        .await
        .unwrap();
    let kept = logs::Entity::find_by_id(fresh)
        .one(&harness.db)
        .await
        .unwrap();
    assert!(gone.is_none());
    assert!(kept.is_some());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn etcd_membership_is_listed_and_unknown_members_rejected() {
    let mut client = etcd().await;
    let members = etcd_members::list_members(&mut client).await.unwrap();
    assert!(etcd_members::voter_count(&members) >= 1);
    let unknown = etcd_members::remove(&mut client, 1).await;
    assert!(matches!(
        unknown,
        Err(etcd_members::MembershipError::NotFound(1))
    ));
}
