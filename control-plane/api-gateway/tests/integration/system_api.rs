use api_gateway::recovery;
use axum::http::{Method, StatusCode};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::json;
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_b::{agent_with_key, AgentSpec};

async fn seed_metric(harness: &Harness, agent_id: Uuid) {
    entity::entities::agent_metrics::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        timestamp: Set(Utc::now().naive_utc()),
        cpu_cores: Set(Some(8)),
        cpu_usage_percent: Set(Some(40.0)),
        memory_total_bytes: Set(Some(1000)),
        memory_used_bytes: Set(Some(250)),
        memory_usage_percent: Set(Some(25.0)),
        disk_total_bytes: Set(Some(2000)),
        disk_used_bytes: Set(Some(500)),
        disk_usage_percent: Set(Some(25.0)),
        uptime_seconds: Set(Some(99)),
        ..Default::default()
    }
    .insert(&harness.db)
    .await
    .unwrap();
}

fn worker() -> AgentSpec {
    AgentSpec {
        role: "worker",
        tunnel_ip: Some("10.250.3.1"),
        age_minutes: 0,
        cordoned: false,
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn health_and_readiness_are_reachable() {
    let harness = harness().await;
    assert_eq!(
        harness.send(Method::GET, "/api/ready", None, None).await.0,
        StatusCode::NO_CONTENT
    );
    let (status, body) = harness.get("/api/system/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn host_information_requires_a_session() {
    let harness = harness().await;
    let (status, info) = harness.get("/api/system/info").await;
    assert_eq!(status, StatusCode::OK);
    assert!(info["cpu_threads"].is_number());
    let (status, metrics) = harness.get("/api/system/metrics").await;
    assert_eq!(status, StatusCode::OK);
    assert!(metrics["metrics"].is_object());
    for uri in [
        "/api/system/info",
        "/api/system/metrics",
        "/api/system/stats",
        "/api/system/stats/history",
        "/api/system/stats/usage",
    ] {
        assert_eq!(
            harness.send(Method::GET, uri, None, None).await.0,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn cluster_stats_aggregate_the_latest_metrics() {
    let harness = harness().await;
    let (agent_id, _) = agent_with_key(&harness, worker()).await;
    seed_metric(&harness, agent_id).await;

    let (status, stats) = harness.get("/api/system/stats").await;
    assert_eq!(status, StatusCode::OK);
    let node = stats["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["agent_id"] == agent_id.to_string())
        .unwrap();
    assert_eq!(node["memory_used_bytes"], 250);
    assert_eq!(node["cpu_usage_percent"], 40.0);
    assert!(stats["total_cpu_cores"].as_i64().unwrap() >= 8);
    assert!(stats["online_count"].as_u64().unwrap() >= 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn history_endpoints_bucket_metrics_by_range() {
    let harness = harness().await;
    let (agent_id, _) = agent_with_key(&harness, worker()).await;
    seed_metric(&harness, agent_id).await;

    for range in ["1h", "24h", "7d", "30d", "bogus"] {
        let (status, points) = harness
            .get(&format!("/api/system/stats/history?range={range}"))
            .await;
        assert_eq!(status, StatusCode::OK, "{range}");
        assert!(points.is_array());
    }
    let (_, points) = harness.get("/api/system/stats/history?range=24h").await;
    assert!(points
        .as_array()
        .unwrap()
        .iter()
        .any(|point| point["online_count"].as_i64().unwrap() >= 1));

    for range in ["1h", "24h", "7d"] {
        let uri = format!("/api/system/stats/usage?range={range}&agent_id={agent_id}");
        let (status, points) = harness.get(&uri).await;
        assert_eq!(status, StatusCode::OK, "{range}");
        assert_eq!(points[0]["cpu"], 40.0);
        assert_eq!(points[0]["memory"], 25.0);
    }
    let (_, everything) = harness.get("/api/system/stats/usage").await;
    assert!(everything.is_array());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ssh_access_and_authorized_keys_follow_the_policy() {
    let harness = harness().await;
    let uri = "/api/admin/settings/ssh";
    let limited = harness.limited_token().await;
    assert_eq!(
        harness.send(Method::GET, uri, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        harness.send(Method::GET, uri, Some(&limited), None).await.0,
        StatusCode::FORBIDDEN
    );

    let key = format!(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI{} test",
        Uuid::new_v4().simple()
    );
    let (status, created) = harness
        .post(
            "/api/ssh-keys",
            json!({"name": "laptop", "public_key": key}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    assert!(created["fingerprint"]
        .as_str()
        .unwrap()
        .starts_with("SHA256:"));
    let expired = format!("ssh-ed25519 AAAAC3expired{} test", Uuid::new_v4().simple());
    let past = (Utc::now() - chrono::Duration::days(1)).to_rfc3339();
    let body = json!({"name": "old", "public_key": expired, "expires_at": past});
    assert_eq!(
        harness.post("/api/ssh-keys", body).await.0,
        StatusCode::CREATED
    );

    let keys_uri = format!("/api/agents/{}/authorized-keys", Uuid::new_v4());
    let (_, closed) = harness.send(Method::GET, &keys_uri, None, None).await;
    assert_eq!(closed["keys"], json!([]));

    let (status, access) = harness
        .admin(
            Method::PUT,
            uri,
            Some(json!({"enabled": true, "duration_hours": 2})),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(access["effective"], true);
    assert!(access["expires_at"].is_string());
    let bad = harness
        .admin(
            Method::PUT,
            uri,
            Some(json!({"enabled": true, "duration_hours": 0})),
        )
        .await;
    assert_eq!(bad.0, StatusCode::BAD_REQUEST);

    let (_, open) = harness.send(Method::GET, &keys_uri, None, None).await;
    let served = open["keys"].as_array().unwrap();
    assert!(served.iter().any(|served| served == &json!(key)));
    assert!(!served.iter().any(|served| served == &json!(expired)));
    let (_, other_user) = harness
        .send(Method::GET, &format!("{keys_uri}?user=root"), None, None)
        .await;
    assert_eq!(other_user["keys"], json!([]));

    let disabled = recovery::dispatch(&["--ssh-disable".to_string()]).await;
    assert_eq!(disabled, Some(0));
    assert_eq!(harness.get(uri).await.1["effective"], false);

    let (_, forever) = harness
        .admin(Method::PUT, uri, Some(json!({"enabled": true})))
        .await;
    assert_eq!(forever["expires_at"], serde_json::Value::Null);
    let (_, off) = harness
        .admin(Method::PUT, uri, Some(json!({"enabled": false})))
        .await;
    assert_eq!(off["effective"], false);
    assert_eq!(harness.get(uri).await.1["enabled"], false);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn ssh_keys_are_private_to_their_owner() {
    let harness = harness().await;
    let key = format!(
        "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAI{} test",
        Uuid::new_v4().simple()
    );
    let (_, created) = harness
        .post("/api/ssh-keys", json!({"name": "mine", "public_key": key}))
        .await;
    let target = format!("/api/ssh-keys/{}", created["id"].as_str().unwrap());

    let (_, listed) = harness.get("/api/ssh-keys").await;
    assert!(listed
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["id"] == created["id"]));

    let (_, _, intruder) = harness.user_with_token().await;
    assert_eq!(
        harness
            .send(Method::DELETE, &target, Some(&intruder), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (_, others) = harness
        .send(Method::GET, "/api/ssh-keys", Some(&intruder), None)
        .await;
    assert_eq!(others, json!([]));
    assert_eq!(
        harness
            .send(Method::GET, "/api/ssh-keys", None, None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );

    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.admin(Method::DELETE, &target, None).await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn update_flow_round_trips_through_etcd() {
    let harness = harness().await;
    for version in ["", "v1;rm", "bad version"] {
        let (status, _) = harness
            .post("/api/system/update", json!({"version": version}))
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{version}");
    }
    let (status, scheduled) = harness
        .post("/api/system/update", json!({"version": "v9.9.9-test"}))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(scheduled["status"], "update_scheduled");

    assert_eq!(
        harness.post("/api/system/update/pause", json!({})).await.0,
        StatusCode::NO_CONTENT
    );
    let (status, paused) = harness.get("/api/system/update/status").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(paused["paused"], true);
    assert_eq!(paused["desired_version"], "v9.9.9-test");

    assert_eq!(
        harness.post("/api/system/update/resume", json!({})).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        harness.get("/api/system/update/status").await.1["paused"],
        false
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn update_and_release_routes_require_system_permission() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let cases = [
        (Method::POST, "/api/system/update"),
        (Method::GET, "/api/system/update/status"),
        (Method::POST, "/api/system/update/pause"),
        (Method::POST, "/api/system/update/resume"),
        (Method::GET, "/api/system/releases"),
    ];
    for (method, uri) in cases {
        let body = Some(json!({"version": "1.0.0"}));
        assert_eq!(
            harness
                .send(method.clone(), uri, None, body.clone())
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
        assert_eq!(
            harness.send(method, uri, Some(&limited), body).await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
}
