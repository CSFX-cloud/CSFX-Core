use axum::http::{Method, StatusCode};
use chrono::Utc;
use entity::entities::{ingress_gateways, ingress_routes};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait,
    PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_a::{
    expect_gateway_workload, insert_agent, insert_api_key, insert_group, insert_workload, keyed,
    plain_harness, prefixed_id, scripted_harness, unique, WorkloadSpec, FAILING_PREFIX,
    MISSING_PREFIX,
};

fn zone() -> String {
    api_gateway::dns_sync::zone_from_env()
}

fn ports() -> Value {
    json!([{"container_port": 8080, "protocol": "tcp", "node_port": 30001}])
}

async fn insert_gateway(
    db: &DatabaseConnection,
    group_id: Uuid,
    workload_id: Uuid,
) -> ingress_gateways::Model {
    ingress_gateways::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        workload_id: Set(workload_id),
        name: Set(unique("gw")),
        http_port: Set(80),
        https_port: Set(443),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(db)
    .await
    .unwrap()
}

async fn insert_route(
    db: &DatabaseConnection,
    group_id: Uuid,
    workload_id: Uuid,
    gateway_id: Uuid,
    hostname: &str,
    tls: bool,
) -> ingress_routes::Model {
    ingress_routes::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(group_id),
        workload_id: Set(workload_id),
        gateway_id: Set(Some(gateway_id)),
        hostname: Set(hostname.to_string()),
        container_port: Set(8080),
        tls: Set(tls),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(db)
    .await
    .unwrap()
}

struct Stack {
    group: entity::entities::resource_groups::Model,
    workload: entity::entities::workloads::Model,
    gateway: ingress_gateways::Model,
}

async fn stack(harness: &Harness) -> Stack {
    let group = insert_group(&harness.db, harness.org_id).await;
    let mut spec = WorkloadSpec::new(group.id, None, "running");
    spec.ports = Some(ports());
    let workload = insert_workload(&harness.db, spec).await;
    let gateway_workload =
        insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "running")).await;
    let gateway = insert_gateway(&harness.db, group.id, gateway_workload.id).await;
    Stack {
        group,
        workload,
        gateway,
    }
}

fn route_request(stack: &Stack, hostname: &str, port: u16) -> Value {
    json!({
        "hostname": hostname,
        "gateway_id": stack.gateway.id,
        "workload_id": stack.workload.id,
        "container_port": port
    })
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ingress_dns_and_gateway_routes_require_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let id = Uuid::new_v4();
    let base = format!("/api/resource-groups/{id}");
    let cases = [
        (Method::GET, format!("{base}/ingress")),
        (Method::POST, format!("{base}/ingress")),
        (Method::DELETE, format!("{base}/ingress/{id}")),
        (Method::GET, format!("{base}/gateways")),
        (Method::POST, format!("{base}/gateways")),
        (Method::DELETE, format!("{base}/gateways/{id}")),
        (Method::GET, format!("{base}/dns-records")),
        (Method::POST, format!("{base}/dns-records")),
        (Method::DELETE, format!("{base}/dns-records/{id}")),
        (Method::GET, "/api/dns/zone".to_string()),
    ];
    for (method, uri) in cases {
        let body = (method == Method::POST).then(|| json!({}));
        let (status, _) = harness
            .send(method.clone(), &uri, Some(&limited), body.clone())
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {uri}");
        let (status, _) = harness.send(method.clone(), &uri, None, body).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{method} {uri}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_groups_return_not_found_for_children() {
    let harness = harness().await;
    let base = format!("/api/resource-groups/{}", Uuid::new_v4());
    for suffix in ["ingress", "gateways", "dns-records"] {
        let (status, _) = harness.get(&format!("{base}/{suffix}")).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{suffix}");
        let (status, _) = harness
            .admin(
                Method::DELETE,
                &format!("{base}/{suffix}/{}", Uuid::new_v4()),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{suffix}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ingress_routes_are_created_listed_and_deleted() {
    let harness = harness().await;
    let stack = stack(&harness).await;
    let uri = format!("/api/resource-groups/{}/ingress", stack.group.id);
    let hostname = format!("App-{}.Example.com", Uuid::new_v4().simple());

    let (status, created) = harness
        .post(&uri, route_request(&stack, &hostname, 8080))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["hostname"], hostname.to_ascii_lowercase());
    assert_eq!(created["tls"], true);
    let route_id = created["id"].as_str().unwrap().to_string();

    let (_, listed) = harness.get(&uri).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    let (status, duplicate) = harness
        .post(&uri, route_request(&stack, &hostname, 8080))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{duplicate}");

    let (status, _) = harness
        .admin(Method::DELETE, &format!("{uri}/{route_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = harness
        .admin(Method::DELETE, &format!("{uri}/{route_id}"), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let remaining = ingress_routes::Entity::find()
        .filter(ingress_routes::Column::ResourceGroupId.eq(stack.group.id))
        .count(&harness.db)
        .await
        .unwrap();
    assert_eq!(remaining, 0);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ingress_creation_validates_its_references() {
    let harness = harness().await;
    let stack = stack(&harness).await;
    let uri = format!("/api/resource-groups/{}/ingress", stack.group.id);
    let valid = format!("ok-{}.example.com", Uuid::new_v4().simple());

    let (status, _) = harness
        .post(&uri, route_request(&stack, "localhost", 8080))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = harness
        .post(&uri, route_request(&stack, &valid, 9999))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut unknown_gateway = route_request(&stack, &valid, 8080);
    unknown_gateway["gateway_id"] = json!(Uuid::new_v4());
    let (status, _) = harness.post(&uri, unknown_gateway).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let mut unknown_workload = route_request(&stack, &valid, 8080);
    unknown_workload["workload_id"] = json!(Uuid::new_v4());
    let (status, _) = harness.post(&uri, unknown_workload).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn gateway_body(name: &str) -> Value {
    json!({"name": name, "http_port": 8080, "https_port": 8443})
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateways_are_scheduled_and_listed() {
    let harness = scripted_harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/gateways", group.id);
    let scheduled =
        insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "pending")).await;
    expect_gateway_workload(group.id, scheduled.id);

    let (status, created) = harness.post(&uri, gateway_body(" Edge ")).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["name"], "edge");
    assert_eq!(created["status"], "pending");
    let expected = format!("edge.{}.{}", group.name.to_ascii_lowercase(), zone());
    assert_eq!(created["endpoint"], expected);

    let (status, duplicate) = harness.post(&uri, gateway_body("edge")).await;
    assert_eq!(status, StatusCode::CONFLICT, "{duplicate}");
    let (_, listed) = harness.get(&uri).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    assert_eq!(listed[0]["id"], created["id"]);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateway_creation_rejects_invalid_requests() {
    let harness = scripted_harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/gateways", group.id);
    for body in [
        json!({"name": "bad_name"}),
        json!({"name": "edge", "http_port": 0}),
        json!({"name": "edge", "http_port": 9000, "https_port": 9000}),
    ] {
        let (status, _) = harness.post(&uri, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateway_creation_fails_without_a_workload_id_from_the_scheduler() {
    let harness = plain_harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/gateways", group.id);
    let (status, _) = harness.post(&uri, gateway_body("edge")).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateway_deletion_follows_the_scheduler_outcome() {
    let harness = scripted_harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let base = format!("/api/resource-groups/{}/gateways", group.id);

    for (prefix, expected, removed) in [
        (0, StatusCode::NO_CONTENT, true),
        (MISSING_PREFIX, StatusCode::NO_CONTENT, true),
        (FAILING_PREFIX, StatusCode::BAD_GATEWAY, false),
    ] {
        let mut spec = WorkloadSpec::new(group.id, None, "running");
        if prefix != 0 {
            spec.id = prefixed_id(prefix);
        }
        let workload = insert_workload(&harness.db, spec).await;
        let gateway = insert_gateway(&harness.db, group.id, workload.id).await;
        let (status, _) = harness
            .admin(Method::DELETE, &format!("{base}/{}", gateway.id), None)
            .await;
        assert_eq!(status, expected);
        let present = ingress_gateways::Entity::find_by_id(gateway.id)
            .one(&harness.db)
            .await
            .unwrap()
            .is_some();
        assert_eq!(present, !removed);
    }
    let (status, _) = harness
        .admin(Method::DELETE, &format!("{base}/{}", Uuid::new_v4()), None)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateway_listing_reports_workload_status() {
    let harness = harness().await;
    let stack = stack(&harness).await;
    let (_, listed) = harness
        .get(&format!("/api/resource-groups/{}/gateways", stack.group.id))
        .await;
    assert_eq!(listed[0]["status"], "running");
}

struct Hosted {
    stack: Stack,
    key: String,
    uri: String,
}

async fn hosted_gateway(harness: &Harness) -> Hosted {
    let stack = stack(harness).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let key = insert_api_key(&harness.db, agent.id).await;
    for id in [stack.gateway.workload_id, stack.workload.id] {
        let model = entity::entities::workloads::Entity::find_by_id(id)
            .one(&harness.db)
            .await
            .unwrap()
            .unwrap();
        let mut active: entity::entities::workloads::ActiveModel = model.into();
        active.assigned_agent_id = Set(Some(agent.id));
        active.update(&harness.db).await.unwrap();
    }
    let uri = format!(
        "/api/resource-groups/{}/gateways/{}/config",
        stack.group.id, stack.gateway.id
    );
    Hosted { stack, key, uri }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateway_config_is_restricted_to_the_hosting_agent() {
    let harness = harness().await;
    let hosted = hosted_gateway(&harness).await;
    let (status, _) = harness.send(Method::GET, &hosted.uri, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let outsider = insert_agent(&harness.db, "worker", "Online").await;
    let outsider_key = insert_api_key(&harness.db, outsider.id).await;
    let (status, _) = keyed(&harness, Method::GET, &hosted.uri, &outsider_key, None).await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let missing = format!(
        "/api/resource-groups/{}/gateways/{}/config",
        hosted.stack.group.id,
        Uuid::new_v4()
    );
    let (status, _) = keyed(&harness, Method::GET, &missing, &hosted.key, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn gateway_config_renders_routes_and_stored_certificate() {
    let harness = harness().await;
    let hosted = hosted_gateway(&harness).await;
    let stack = &hosted.stack;
    let internal = format!("app-{}.{}", Uuid::new_v4().simple(), zone());
    let plain = insert_route(
        &harness.db,
        stack.group.id,
        stack.workload.id,
        stack.gateway.id,
        &format!("plain-{}.example.com", Uuid::new_v4().simple()),
        false,
    )
    .await;
    let secure = insert_route(
        &harness.db,
        stack.group.id,
        stack.workload.id,
        stack.gateway.id,
        &internal,
        true,
    )
    .await;
    store_certificate(&harness, stack.gateway.id, &internal).await;

    let (status, config) = keyed(&harness, Method::GET, &hosted.uri, &hosted.key, None).await;
    assert_eq!(status, StatusCode::OK, "{config}");
    let routers = &config["http"]["routers"];
    assert!(routers[format!("csfx-{}", plain.id)]["entryPoints"][0] == "web");
    assert!(routers[format!("csfx-{}", secure.id)]["entryPoints"][0] == "websecure");
    assert!(routers[format!("csfx-{}-redirect", secure.id)].is_object());
    let backend = &config["http"]["services"][format!("csfx-{}", plain.id)];
    assert_eq!(
        backend["loadBalancer"]["servers"][0]["url"],
        "http://127.0.0.1:30001"
    );
    assert_eq!(config["tls"]["certificates"][0]["certFile"], "cert");
}

async fn store_certificate(harness: &Harness, gateway_id: Uuid, hostname: &str) {
    let certificate = api_gateway::zone_cert::IssuedCertificate {
        sans: vec![hostname.to_string()],
        cert_pem: "cert".to_string(),
        key_pem: "key".to_string(),
        expires_on: Utc::now().date_naive() + chrono::Duration::days(80),
    };
    entity::entities::system_settings::ActiveModel {
        key: Set(format!("ingress.certificate.{gateway_id}")),
        value: Set(serde_json::to_value(certificate).unwrap()),
        updated_at: Set(Utc::now().fixed_offset()),
    }
    .insert(&harness.db)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn deleting_a_gateway_removes_its_certificate() {
    let harness = scripted_harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let workload = insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "running")).await;
    let gateway = insert_gateway(&harness.db, group.id, workload.id).await;
    store_certificate(&harness, gateway.id, "x.example.com").await;
    let (status, _) = harness
        .admin(
            Method::DELETE,
            &format!("/api/resource-groups/{}/gateways/{}", group.id, gateway.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let stored = entity::entities::system_settings::Entity::find_by_id(format!(
        "ingress.certificate.{}",
        gateway.id
    ))
    .one(&harness.db)
    .await
    .unwrap();
    assert!(stored.is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn dns_zone_is_reported() {
    let harness = harness().await;
    let (status, body) = harness.get("/api/dns/zone").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["zone"], zone());
    assert!(body["console_host"].as_str().unwrap().ends_with(&zone()));
    assert!(body["servers"].is_array());
}

fn record(name: &str, record_type: &str, value: &str) -> Value {
    json!({"name": name, "record_type": record_type, "value": value})
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn dns_records_are_created_listed_and_deleted() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/dns-records", group.id);
    let name = format!("rec-{}.{}", Uuid::new_v4().simple(), zone());

    let (status, created) = harness.post(&uri, record(&name, "a", "10.1.2.3")).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["record_type"], "A");
    assert_eq!(created["ttl"], 30);
    let (_, listed) = harness.get(&uri).await;
    assert_eq!(listed.as_array().unwrap().len(), 1);
    let (status, _) = harness.post(&uri, record(&name, "A", "10.1.2.4")).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let delete = format!("{uri}/{}", created["id"].as_str().unwrap());
    assert_eq!(
        harness.admin(Method::DELETE, &delete, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.admin(Method::DELETE, &delete, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn dns_records_validate_their_input() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/dns-records", group.id);
    let name = format!("val-{}.{}", Uuid::new_v4().simple(), zone());
    let mut slow = record(&name, "A", "10.1.2.3");
    slow["ttl"] = json!(1);

    for body in [
        record("outside.example.com", "A", "10.1.2.3"),
        record(&name, "A", "999.1.1.1"),
        record(&name, "TXT", "text"),
        record(&name, "CNAME", &name),
        slow,
    ] {
        let (status, _) = harness.post(&uri, body).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, cname) = harness
        .post(&uri, record(&name, "CNAME", "Target.Example.com"))
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(cname["value"], "target.example.com");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn dns_names_cannot_shadow_ingress_hostnames() {
    let harness = harness().await;
    let stack = stack(&harness).await;
    let hostname = format!("shadow-{}.{}", Uuid::new_v4().simple(), zone());
    insert_route(
        &harness.db,
        stack.group.id,
        stack.workload.id,
        stack.gateway.id,
        &hostname,
        false,
    )
    .await;
    let uri = format!("/api/resource-groups/{}/dns-records", stack.group.id);
    let (status, body) = harness.post(&uri, record(&hostname, "A", "10.1.2.3")).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "name is already in use");
}
