use axum::http::{Method, StatusCode};
use entity::entities::{agent_metrics, agents, logs, workloads};
use futures_util::StreamExt;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_a::{
    insert_agent, insert_api_key, insert_group, insert_volume, insert_workload, keyed, serve,
    update_agent, WorkloadSpec,
};

async fn worker(harness: &Harness) -> agents::Model {
    insert_agent(&harness.db, "worker", "Online").await
}

fn metrics_payload() -> Value {
    json!({
        "cpu_model": "test-cpu",
        "cpu_cores": 4,
        "memory_total_bytes": 1000,
        "memory_used_bytes": 250,
        "disk_total_bytes": 0,
        "disk_used_bytes": 10,
        "hostname": "metrics-host"
    })
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_admin_routes_require_permissions() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let id = Uuid::new_v4();
    let cases = [
        (Method::GET, "/api/agents".to_string()),
        (Method::GET, format!("/api/agents/{id}")),
        (Method::GET, format!("/api/agents/{id}/metrics")),
        (Method::GET, format!("/api/agents/{id}/metrics/latest")),
        (Method::POST, format!("/api/agents/{id}/cordon")),
        (Method::POST, format!("/api/agents/{id}/uncordon")),
        (Method::POST, format!("/api/agents/{id}/drain")),
        (Method::PUT, format!("/api/agents/{id}/overcommit")),
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
async fn agents_are_listed_and_fetched_without_container_hosts() {
    let harness = harness().await;
    let visible = worker(&harness).await;
    let container = update_agent(&harness.db, worker(&harness).await, |agent| {
        agent.hostname = Set("abcdef012345".to_string());
    })
    .await;

    let (status, listed) = harness.get("/api/agents").await;
    assert_eq!(status, StatusCode::OK);
    let ids: Vec<&str> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|agent| agent["id"].as_str().unwrap())
        .collect();
    assert!(ids.contains(&visible.id.to_string().as_str()));
    assert!(!ids.contains(&container.id.to_string().as_str()));

    let (status, detail) = harness.get(&format!("/api/agents/{}", visible.id)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["hostname"], visible.hostname);
    assert_eq!(detail["cordoned"], false);
    let (status, _) = harness
        .get(&format!("/api/agents/{}", Uuid::new_v4()))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pushed_metrics_are_stored_and_served() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let (status, _) = harness
        .send(
            Method::POST,
            "/api/agents/self/metrics",
            None,
            Some(metrics_payload()),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let other = worker(&harness).await;
    let (status, _) = keyed(
        &harness,
        Method::POST,
        "/api/agents/self/metrics",
        &key,
        Some(json!({ "agent_id": other.id, "timestamp": "2999-01-01T00:00:00Z", "cpu_cores": 0 })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let foreign = agent_metrics::Entity::find()
        .filter(agent_metrics::Column::AgentId.eq(other.id))
        .count(&harness.db)
        .await
        .unwrap();
    assert_eq!(foreign, 0);
    agent_metrics::Entity::delete_many()
        .filter(agent_metrics::Column::AgentId.eq(agent.id))
        .exec(&harness.db)
        .await
        .unwrap();
    let (status, _) = keyed(
        &harness,
        Method::POST,
        "/api/agents/self/metrics",
        &key,
        Some(metrics_payload()),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let (status, history) = harness
        .get(&format!("/api/agents/{}/metrics", agent.id))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(history.as_array().unwrap().len(), 1);
    let (status, latest) = harness
        .get(&format!("/api/agents/{}/metrics/latest", agent.id))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(latest["memory_usage_percent"], 25.0);
    assert!(latest["disk_usage_percent"].is_null());
    let stored = agent_metrics::Entity::find()
        .filter(agent_metrics::Column::AgentId.eq(agent.id))
        .count(&harness.db)
        .await
        .unwrap();
    assert_eq!(stored, 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn latest_metrics_of_an_unknown_agent_are_missing() {
    let harness = harness().await;
    let uri = format!("/api/agents/{}/metrics/latest", Uuid::new_v4());
    assert_eq!(harness.get(&uri).await.0, StatusCode::NOT_FOUND);
    let (status, _) = harness
        .send(
            Method::POST,
            "/api/agents/self/metrics",
            None,
            Some(json!({})),
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn cordon_and_uncordon_toggle_the_flag() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    for (action, expected) in [("cordon", true), ("uncordon", false)] {
        let (status, _) = harness
            .admin(
                Method::POST,
                &format!("/api/agents/{}/{action}", agent.id),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        let stored = agents::Entity::find_by_id(agent.id)
            .one(&harness.db)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.cordoned, expected);
        let missing = format!("/api/agents/{}/{action}", Uuid::new_v4());
        let (status, _) = harness.admin(Method::POST, &missing, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn drain_cordons_the_agent_and_reschedules() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let (status, _) = harness
        .admin(
            Method::POST,
            &format!("/api/agents/{}/drain", agent.id),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let stored = agents::Entity::find_by_id(agent.id)
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    assert!(stored.cordoned);
    let missing = format!("/api/agents/{}/drain", Uuid::new_v4());
    let (status, _) = harness.admin(Method::POST, &missing, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn overcommit_ratios_are_validated_and_stored() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let uri = format!("/api/agents/{}/overcommit", agent.id);
    for invalid in [
        json!({"cpu_ratio": 0.5}),
        json!({"cpu_ratio": 17.0}),
        json!({"memory_ratio": 4.5}),
        json!({"disk_ratio": 0.0}),
    ] {
        let (status, _) = harness.admin(Method::PUT, &uri, Some(invalid)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }

    let body = json!({"cpu_ratio": 4.0, "memory_ratio": 2.0, "disk_ratio": 3.0});
    let (status, _) = harness.admin(Method::PUT, &uri, Some(body)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let stored = agents::Entity::find_by_id(agent.id)
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            stored.cpu_overcommit_ratio,
            stored.memory_overcommit_ratio,
            stored.disk_overcommit_ratio
        ),
        (4.0, 2.0, 3.0)
    );
    let missing = format!("/api/agents/{}/overcommit", Uuid::new_v4());
    let (status, _) = harness.admin(Method::PUT, &missing, Some(json!({}))).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn self_routes_require_an_agent_key() {
    let harness = harness().await;
    let id = Uuid::new_v4();
    for (method, uri) in [
        (Method::GET, "/api/agents/self/workloads".to_string()),
        (Method::GET, "/api/agents/self/volumes".to_string()),
        (Method::POST, "/api/agents/self/workloads/stats".to_string()),
        (
            Method::POST,
            format!("/api/agents/self/workloads/{id}/restart-ack"),
        ),
        (Method::POST, "/api/agents/self/logs".to_string()),
    ] {
        let (status, _) = harness
            .send(method.clone(), &uri, None, Some(json!({})))
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn self_workloads_include_group_cidr_and_skip_stopped() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let group = insert_group(&harness.db, harness.org_id).await;
    let grouped = insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "running"),
    )
    .await;
    let mut loose_spec = WorkloadSpec::new(group.id, Some(agent.id), "running");
    loose_spec.group_id = None;
    let loose = insert_workload(&harness.db, loose_spec).await;
    let stopped = insert_workload(
        &harness.db,
        WorkloadSpec::new(group.id, Some(agent.id), "running"),
    )
    .await;
    let mut active: workloads::ActiveModel = stopped.into();
    active.desired_state = Set("stopped".to_string());
    active.update(&harness.db).await.unwrap();

    let (status, rows) = keyed(
        &harness,
        Method::GET,
        "/api/agents/self/workloads",
        &key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let rows = rows.as_array().unwrap();
    assert_eq!(rows.len(), 2);
    let find = |id: Uuid| rows.iter().find(|row| row["id"] == id.to_string()).unwrap();
    assert_eq!(find(grouped.id)["resource_group_cidr"], group.internal_cidr);
    assert!(find(loose.id)["resource_group_cidr"].is_null());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn self_volumes_list_attached_volumes() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let volume = insert_volume(&harness.db, Some(agent.id), None).await;

    let (status, rows) = keyed(
        &harness,
        Method::GET,
        "/api/agents/self/volumes",
        &key,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["id"], volume.id.to_string());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn self_stats_and_restart_ack_are_forwarded() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let (status, _) = keyed(
        &harness,
        Method::POST,
        "/api/agents/self/workloads/stats",
        &key,
        Some(json!({"stats": []})),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let ack = format!("/api/agents/self/workloads/{}/restart-ack", Uuid::new_v4());
    let (status, _) = keyed(&harness, Method::POST, &ack, &key, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn agent_logs_are_validated_normalized_and_stored() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let uri = "/api/agents/self/logs";

    let (status, _) = keyed(
        &harness,
        Method::POST,
        uri,
        &key,
        Some(json!({"entries": []})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let oversized: Vec<Value> = (0..201)
        .map(|_| json!({"level": "ERROR", "unit": "u", "message": "m"}))
        .collect();
    let (status, _) = keyed(
        &harness,
        Method::POST,
        uri,
        &key,
        Some(json!({"entries": oversized})),
    )
    .await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);

    let entries = json!({"entries": [
        {"level": "ERROR", "unit": "sshd", "message": "x".repeat(5000), "timestamp_micros": 1_700_000_000_000_000i64},
        {"level": "INFO", "unit": "cron", "message": "short"}
    ]});
    let (status, _) = keyed(&harness, Method::POST, uri, &key, Some(entries)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let stored = logs::Entity::find()
        .filter(logs::Column::AgentId.eq(agent.id))
        .all(&harness.db)
        .await
        .unwrap();
    assert_eq!(stored.len(), 2);
    let long = stored.iter().find(|row| row.level == "ERROR").unwrap();
    assert_eq!(long.message.chars().count(), 4000);
    assert_eq!(long.fields["unit"], "sshd");
    assert_eq!(long.created_at.timestamp(), 1_700_000_000);
    assert!(stored.iter().any(|row| row.level == "WARN"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn notify_assignment_is_limited_to_internal_peers() {
    let harness = harness().await;
    let uri = format!("/api/internal/agents/{}/notify-assignment", Uuid::new_v4());
    for peer in ["8.8.8.8:5000", "[2001:db8::1]:5000"] {
        let (status, _) = harness
            .send_from(peer, Method::POST, &uri, None, None)
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{peer}");
    }
    for peer in ["127.0.0.1:1", "10.1.2.3:1", "172.16.0.9:1", "192.168.1.1:1"] {
        let (status, _) = harness
            .send_from(peer, Method::POST, &uri, None, None)
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT, "{peer}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn connected_agent_streams_receive_assignment_signals() {
    let harness = harness().await;
    let agent = worker(&harness).await;
    let key = insert_api_key(&harness.db, agent.id).await;
    let address = serve(&harness).await;

    let mut request = format!("ws://{address}/api/internal/agent-stream")
        .into_client_request()
        .unwrap();
    assert!(tokio_tungstenite::connect_async(request.clone())
        .await
        .is_err());
    request
        .headers_mut()
        .insert("x-api-key", key.parse().unwrap());
    let (mut socket, _) = tokio_tungstenite::connect_async(request).await.unwrap();

    let notify = format!("/api/internal/agents/{}/notify-assignment", agent.id);
    for _ in 0..50 {
        harness.admin(Method::POST, &notify, None).await;
        let next = tokio::time::timeout(std::time::Duration::from_millis(200), socket.next()).await;
        if let Ok(Some(Ok(message))) = next {
            assert_eq!(message, Message::Text("assignment".into()));
            return;
        }
    }
    panic!("assignment signal was never delivered");
}
