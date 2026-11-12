use axum::http::{Method, StatusCode};
use entity::entities::{resource_group_vpn_peers, resource_groups};
use sea_orm::{
    ActiveModelTrait, ActiveValue, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_a::{
    insert_agent, insert_api_key, insert_group, insert_workload, keyed, random_cidr, raw, unique,
    WorkloadSpec,
};

async fn create_group(harness: &Harness, cidr: &str) -> Value {
    let (status, body) = harness
        .post(
            "/api/resource-groups",
            json!({"name": unique("rg"), "internal_cidr": cidr}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body
}

async fn remove_group(harness: &Harness, id: Uuid) {
    resource_groups::Entity::delete_by_id(id)
        .exec(&harness.db)
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn resource_group_routes_require_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let id = Uuid::new_v4();
    let cases = [
        (Method::GET, "/api/resource-groups".to_string()),
        (Method::POST, "/api/resource-groups".to_string()),
        (Method::GET, "/api/resource-groups/suggest-cidr".to_string()),
        (Method::GET, format!("/api/resource-groups/{id}")),
        (Method::PATCH, format!("/api/resource-groups/{id}")),
        (Method::DELETE, format!("/api/resource-groups/{id}")),
        (Method::GET, format!("/api/resource-groups/{id}/icon-image")),
        (
            Method::DELETE,
            format!("/api/resource-groups/{id}/icon-image"),
        ),
        (Method::GET, format!("/api/resource-groups/{id}/workloads")),
        (Method::GET, format!("/api/resource-groups/{id}/volumes")),
        (Method::GET, format!("/api/resource-groups/{id}/buckets")),
        (Method::GET, format!("/api/resource-groups/{id}/networks")),
        (
            Method::POST,
            format!("/api/resource-groups/{id}/vpn-config"),
        ),
        (
            Method::DELETE,
            format!("/api/resource-groups/{id}/vpn-peers/{}", Uuid::new_v4()),
        ),
    ];
    for (method, uri) in cases {
        let body = (method != Method::GET).then(|| json!({}));
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
async fn resource_group_lifecycle_persists_changes() {
    let harness = harness().await;
    let created = create_group(&harness, &random_cidr()).await;
    let id: Uuid = created["id"].as_str().unwrap().parse().unwrap();
    assert_eq!(created["status"], "active");
    assert_eq!(created["icon"], "mdi:cube-outline");
    assert_eq!(created["pinned"], false);

    let (_, listed) = harness.get("/api/resource-groups").await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["id"] == created["id"]));

    let (status, updated) = harness
        .admin(
            Method::PATCH,
            &format!("/api/resource-groups/{id}"),
            Some(json!({
                "name": "renamed", "description": "d", "color": "#fff",
                "pinned": true, "preview_url": " https://example.com "
            })),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(updated["pinned"], true);
    assert_eq!(updated["preview_url"], "https://example.com");
    let stored = resource_groups::Entity::find_by_id(id)
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.name, "renamed");
    assert_eq!(stored.description.as_deref(), Some("d"));

    let (status, cleared) = harness
        .admin(
            Method::PATCH,
            &format!("/api/resource-groups/{id}"),
            Some(json!({"preview_url": "  ", "icon": "mdi:star"})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(cleared["preview_url"].is_null());
    assert_eq!(cleared["icon"], "mdi:star");

    let (status, _) = harness
        .admin(Method::DELETE, &format!("/api/resource-groups/{id}"), None)
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = harness.get(&format!("/api/resource-groups/{id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn resource_group_creation_validates_the_cidr() {
    let harness = harness().await;
    let existing = create_group(&harness, &random_cidr()).await;
    let cidr = existing["internal_cidr"].as_str().unwrap();
    let id: Uuid = existing["id"].as_str().unwrap().parse().unwrap();

    for (bad, expected) in [
        ("not-a-cidr", StatusCode::BAD_REQUEST),
        ("10.1.2.3/24", StatusCode::BAD_REQUEST),
        ("10.1.2.0/33", StatusCode::BAD_REQUEST),
        ("10.1.2/24", StatusCode::BAD_REQUEST),
        ("10.100.5.0/24", StatusCode::CONFLICT),
        ("0.0.0.0/0", StatusCode::BAD_REQUEST),
        ("8.8.8.0/24", StatusCode::BAD_REQUEST),
        ("172.32.0.0/16", StatusCode::BAD_REQUEST),
        ("10.0.0.0/8", StatusCode::BAD_REQUEST),
        ("192.168.0.0/15", StatusCode::BAD_REQUEST),
        ("192.168.7.0/30", StatusCode::BAD_REQUEST),
        (cidr, StatusCode::CONFLICT),
    ] {
        let (status, body) = harness
            .post(
                "/api/resource-groups",
                json!({"name": unique("rg"), "internal_cidr": bad}),
            )
            .await;
        assert_eq!(status, expected, "{bad}");
        assert!(body["error"].is_string());
    }
    remove_group(&harness, id).await;
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn resource_group_creation_rejects_a_cidr_containing_a_node_address() {
    let harness = harness().await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let lan = format!("172.20.{}", agent.id.as_bytes()[0]);
    let mut active: entity::entities::agents::ActiveModel = agent.into();
    active.ip_address = ActiveValue::Set(Some(format!("{lan}.5")));
    active.update(&harness.db).await.unwrap();

    let (status, body) = harness
        .post(
            "/api/resource-groups",
            json!({"name": unique("rg"), "internal_cidr": format!("{lan}.0/24")}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert!(body["error"].as_str().unwrap().contains("node"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn suggest_cidr_returns_a_free_subnet() {
    let harness = harness().await;
    let (status, body) = harness.get("/api/resource-groups/suggest-cidr").await;
    assert_eq!(status, StatusCode::OK);
    let cidr = body["internal_cidr"].as_str().unwrap();
    assert!(
        cidr.starts_with("10.101.") && cidr.ends_with("/24"),
        "{cidr}"
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_resource_groups_return_not_found() {
    let harness = harness().await;
    let id = Uuid::new_v4();
    for suffix in [
        "",
        "/workloads",
        "/volumes",
        "/buckets",
        "/networks",
        "/icon-image",
    ] {
        let (status, _) = harness
            .get(&format!("/api/resource-groups/{id}{suffix}"))
            .await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{suffix}");
    }
    for (method, uri) in [
        (Method::PATCH, format!("/api/resource-groups/{id}")),
        (Method::DELETE, format!("/api/resource-groups/{id}")),
        (
            Method::DELETE,
            format!("/api/resource-groups/{id}/icon-image"),
        ),
    ] {
        let (status, _) = harness.admin(method, &uri, Some(json!({}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let (status, _, _) = raw(
        &harness,
        Method::PUT,
        &format!("/api/resource-groups/{id}/icon-image"),
        &[
            ("authorization", &format!("Bearer {}", harness.token)),
            ("content-type", "image/png"),
        ],
        vec![1, 2, 3],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = harness
        .post(&format!("/api/resource-groups/{id}/vpn-config"), json!({}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn update_rejects_a_preview_url_without_scheme() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let (status, _) = harness
        .admin(
            Method::PATCH,
            &format!("/api/resource-groups/{}", group.id),
            Some(json!({"preview_url": "ftp://example.com"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

async fn put_icon(harness: &Harness, id: Uuid, mime: &str, body: Vec<u8>) -> StatusCode {
    let (status, _, _) = raw(
        harness,
        Method::PUT,
        &format!("/api/resource-groups/{id}/icon-image"),
        &[
            ("authorization", &format!("Bearer {}", harness.token)),
            ("content-type", mime),
        ],
        body,
    )
    .await;
    status
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn icon_image_is_validated_stored_served_and_removed() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/icon-image", group.id);

    assert_eq!(
        put_icon(&harness, group.id, "text/plain", vec![1]).await,
        StatusCode::UNSUPPORTED_MEDIA_TYPE
    );
    assert_eq!(
        put_icon(&harness, group.id, "image/png", vec![0; 512 * 1024 + 1]).await,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(harness.get(&uri).await.0, StatusCode::NOT_FOUND);

    assert_eq!(
        put_icon(&harness, group.id, "image/png", vec![7, 8, 9]).await,
        StatusCode::OK
    );
    let (_, detail) = harness
        .get(&format!("/api/resource-groups/{}", group.id))
        .await;
    assert_eq!(detail["has_icon_image"], true);
    let (status, headers, bytes) = raw(
        &harness,
        Method::GET,
        &uri,
        &[("authorization", &format!("Bearer {}", harness.token))],
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-type"], "image/png");
    assert_eq!(bytes.as_ref(), [7, 8, 9]);

    let (status, removed) = harness.admin(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(removed["has_icon_image"], false);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn group_listings_return_only_their_own_children() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let workload = insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "running"),
    )
    .await;

    let (status, workloads) = harness
        .get(&format!("/api/resource-groups/{}/workloads", group.id))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(workloads.as_array().unwrap().len(), 1);
    assert_eq!(workloads[0]["id"], workload.id.to_string());
    for suffix in ["volumes", "buckets", "networks"] {
        let (status, rows) = harness
            .get(&format!("/api/resource-groups/{}/{suffix}", group.id))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(rows, json!([]));
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn deleting_a_group_with_active_workloads_conflicts() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    insert_workload(&harness.db, WorkloadSpec::new(group.id, None, "running")).await;

    let (status, body) = harness
        .admin(
            Method::DELETE,
            &format!("/api/resource-groups/{}", group.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"], "resource group has active workloads");
    let remaining = resource_groups::Entity::find_by_id(group.id)
        .count(&harness.db)
        .await
        .unwrap();
    assert_eq!(remaining, 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_config_requires_an_online_wireguard_agent() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let uri = format!("/api/resource-groups/{}/vpn-config", group.id);
    assert_eq!(
        harness.post(&uri, json!({})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );

    let offline = insert_agent(&harness.db, "worker", "Offline").await;
    insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(offline.id), "running"),
    )
    .await;
    assert_eq!(
        harness.post(&uri, json!({})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_config_allocates_distinct_peers() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let prefix = group.internal_cidr.trim_end_matches(".0/24").to_string();
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "running"),
    )
    .await;
    let uri = format!("/api/resource-groups/{}/vpn-config", group.id);

    let mut configs = Vec::new();
    for _ in 0..2 {
        let (status, headers, bytes) = raw(
            &harness,
            Method::POST,
            &uri,
            &[("authorization", &format!("Bearer {}", harness.token))],
            Vec::new(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert!(headers["content-disposition"]
            .to_str()
            .unwrap()
            .contains(".conf"));
        configs.push(String::from_utf8(bytes.to_vec()).unwrap());
    }
    assert!(configs[0].contains(&format!("Address = {prefix}.254/32")));
    assert!(configs[1].contains(&format!("Address = {prefix}.253/32")));
    assert!(configs[0].contains(&format!("DNS = {prefix}.2, svc.")));
    assert!(configs[0].contains(&format!("AllowedIPs = {prefix}.0/24")));
    let peers = resource_group_vpn_peers::Entity::find()
        .filter(resource_group_vpn_peers::Column::ResourceGroupId.eq(group.id))
        .all(&harness.db)
        .await
        .unwrap();
    assert_eq!(peers.len(), 2);
    assert!(peers.iter().all(|peer| peer.user_id == harness.admin_id));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_routes_reject_missing_or_unknown_keys() {
    let harness = harness().await;
    let id = Uuid::new_v4();
    for uri in [
        "/api/resource-groups/agent/active-ids".to_string(),
        "/api/agents/mesh-peers".to_string(),
        format!("/api/resource-groups/{id}/peers"),
        format!("/api/resource-groups/{id}/members"),
        format!("/api/resource-groups/{id}/site-services"),
        format!("/api/resource-groups/{id}/vpn-peers"),
    ] {
        let (status, _) = harness.send(Method::GET, &uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
        let (status, _) = keyed(&harness, Method::GET, &uri, "unknown-key", None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agents_see_group_peers_members_and_mesh() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let mut spec = WorkloadSpec::new(group.id, Some(agent.id), "running");
    spec.guest_ip = Some("10.9.9.9".to_string());
    let workload = insert_workload(&harness.db, spec).await;
    insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "failed"),
    )
    .await;

    let (status, ids) = keyed(
        &harness,
        Method::GET,
        "/api/resource-groups/agent/active-ids",
        &key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(ids.as_array().unwrap().contains(&json!(group.id)));

    let (status, peers) = keyed(
        &harness,
        Method::GET,
        &format!("/api/resource-groups/{}/peers", group.id),
        &key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(peers.as_array().unwrap().len(), 1);
    assert_eq!(peers[0]["agent_id"], agent.id.to_string());

    let (status, members) = keyed(
        &harness,
        Method::GET,
        &format!("/api/resource-groups/{}/members", group.id),
        &key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(members.as_array().unwrap().len(), 1);
    assert_eq!(members[0]["workload_id"], workload.id.to_string());
    assert_eq!(members[0]["guest_ip"], "10.9.9.9");

    let (status, mesh) = keyed(&harness, Method::GET, "/api/agents/mesh-peers", &key, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(mesh
        .as_array()
        .unwrap()
        .iter()
        .any(|peer| peer["agent_id"] == agent.id.to_string()));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn peers_of_an_empty_or_unknown_group() {
    let harness = harness().await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let base = format!("/api/resource-groups/{}", group.id);

    let (status, peers) = keyed(&harness, Method::GET, &format!("{base}/peers"), &key, None).await;
    assert_eq!((status, peers), (StatusCode::OK, json!([])));
    let (status, _) = keyed(
        &harness,
        Method::GET,
        &format!("/api/resource-groups/{}/peers", Uuid::new_v4()),
        &key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, services) = keyed(
        &harness,
        Method::GET,
        &format!("{base}/site-services"),
        &key,
        None,
    )
    .await;
    assert_eq!((status, services), (StatusCode::OK, json!([])));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_peers_are_listed_for_agents() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let agent = insert_agent(&harness.db, "worker", "Online").await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let uri = format!("/api/resource-groups/{}/vpn-peers", group.id);
    assert_eq!(
        keyed(&harness, Method::GET, &uri, &key, None).await.1,
        json!([])
    );

    ActiveModelTrait::insert(
        resource_group_vpn_peers::ActiveModel {
            id: ActiveValue::Set(Uuid::new_v4()),
            resource_group_id: ActiveValue::Set(group.id),
            user_id: ActiveValue::Set(harness.admin_id),
            client_public_key: ActiveValue::Set("client-key".to_string()),
            client_tunnel_ip: ActiveValue::Set("10.1.1.9".to_string()),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        },
        &harness.db,
    )
    .await
    .unwrap();
    let (_, peers) = keyed(&harness, Method::GET, &uri, &key, None).await;
    assert_eq!(peers[0]["client_public_key"], "client-key");
    assert_eq!(peers[0]["client_tunnel_ip"], "10.1.1.9");
}

async fn insert_owned_peer(harness: &Harness, group_id: Uuid, user_id: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    ActiveModelTrait::insert(
        resource_group_vpn_peers::ActiveModel {
            id: ActiveValue::Set(id),
            resource_group_id: ActiveValue::Set(group_id),
            user_id: ActiveValue::Set(user_id),
            client_public_key: ActiveValue::Set(unique("key")),
            client_tunnel_ip: ActiveValue::Set("10.1.1.9".to_string()),
            created_at: ActiveValue::Set(chrono::Utc::now().naive_utc()),
        },
        &harness.db,
    )
    .await
    .unwrap();
    id
}

async fn peers_of(harness: &Harness, user_id: Uuid) -> u64 {
    resource_group_vpn_peers::Entity::find()
        .filter(resource_group_vpn_peers::Column::UserId.eq(user_id))
        .count(&harness.db)
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_config_is_denied_to_viewers() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let (_, viewer) = harness
        .token_with_permissions(&["resource_groups.view"])
        .await;
    let uri = format!("/api/resource-groups/{}/vpn-config", group.id);
    let (status, _) = harness
        .send(Method::POST, &uri, Some(&viewer), Some(json!({})))
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = harness.send(Method::GET, &uri, Some(&viewer), None).await;
    assert_ne!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_peer_can_be_revoked() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let peer_id = insert_owned_peer(&harness, group.id, harness.admin_id).await;
    let uri = format!("/api/resource-groups/{}/vpn-peers/{peer_id}", group.id);
    let (status, _) = harness.admin(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = harness.admin(Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_peers_are_removed_with_their_owner() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let (owner_id, _, _) = harness.user_with_token().await;
    insert_owned_peer(&harness, group.id, owner_id).await;
    assert_eq!(peers_of(&harness, owner_id).await, 1);
    let (status, _) = harness
        .admin(
            Method::DELETE,
            &format!("/api/organization/users/{owner_id}"),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(peers_of(&harness, owner_id).await, 0);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn vpn_peers_are_removed_when_manage_permission_is_lost() {
    let harness = harness().await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let (owner_id, _) = harness
        .token_with_permissions(&["resource_groups.manage"])
        .await;
    insert_owned_peer(&harness, group.id, owner_id).await;
    let viewer_role = harness
        .role_with_permissions(&["resource_groups.view"])
        .await;
    let (status, _) = harness
        .admin(
            Method::PUT,
            &format!("/api/organization/users/{owner_id}/role"),
            Some(json!({"role_id": viewer_role})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(peers_of(&harness, owner_id).await, 0);
}
