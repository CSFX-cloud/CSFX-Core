use axum::http::{Method, StatusCode};
use axum::routing::any;
use axum::{Json, Router};
use chrono::Utc;
use sea_orm::{ActiveModelTrait, ActiveValue::Set, EntityTrait};
use serde_json::json;
use uuid::Uuid;

use crate::common::Harness;
use crate::support_b::{agent_with_key, full_harness, raw, AgentSpec};

async fn insert_bucket(harness: &Harness, exposure: &str) -> String {
    let id = Uuid::new_v4();
    let alias = format!("bucket-{}", id.simple());
    entity::entities::buckets::ActiveModel {
        id: Set(id),
        name: Set(alias.clone()),
        garage_bucket_id: Set(None),
        global_alias: Set(alias.clone()),
        exposure: Set(exposure.to_string()),
        quota_max_size: Set(None),
        quota_max_objects: Set(None),
        status: Set("active".to_string()),
        master_key_id: Set(None),
        master_key_secret_encrypted: Set(None),
        organization_id: Set(None),
        resource_group_id: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    }
    .insert(&harness.db)
    .await
    .unwrap();
    alias
}

async fn insert_node(harness: &Harness, agent_id: Option<Uuid>) {
    entity::entities::garage_nodes::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(agent_id),
        garage_node_id: Set(None),
        zone: Set("zone".to_string()),
        capacity_bytes: Set(None),
        role: Set("storage".to_string()),
        status: Set("up".to_string()),
        layout_version: Set(None),
        last_seen_at: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    }
    .insert(&harness.db)
    .await
    .unwrap();
}

async fn clear_nodes(harness: &Harness) {
    entity::entities::garage_nodes::Entity::delete_many()
        .exec(&harness.db)
        .await
        .unwrap();
}

async fn fetch(harness: &Harness, uri: &str) -> (StatusCode, serde_json::Value) {
    let response = raw(harness, Method::GET, uri, &[], None).await;
    (response.status, response.body)
}

async fn garage_stub() {
    let echo = |uri: axum::http::Uri| async move { Json(json!({"target": uri.to_string()})) };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3900")
        .await
        .expect("port 3900 must be free for the garage stub");
    tokio::spawn(async move {
        axum::serve(listener, Router::new().fallback(any(echo)))
            .await
            .unwrap();
    });
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn s3_requests_resolve_buckets_and_garage_nodes() {
    let harness = full_harness().await;
    clear_nodes(&harness).await;

    let (status, _) = fetch(&harness, "/api/s3/missing-bucket/object").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let internal = insert_bucket(&harness, "internal").await;
    let external = insert_bucket(&harness, "external").await;
    let (status, _) = fetch(&harness, &format!("/api/s3/{internal}/object")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, _) = fetch(&harness, &format!("/api/s3/{external}/object")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    let (agent_id, _) = agent_with_key(
        &harness,
        AgentSpec {
            role: "worker",
            tunnel_ip: None,
            age_minutes: 0,
            cordoned: false,
        },
    )
    .await;
    insert_node(&harness, Some(agent_id)).await;
    let (status, _) = fetch(&harness, &format!("/api/s3/{external}/object")).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);

    clear_nodes(&harness).await;
    let (agent_id, _) = agent_with_key(
        &harness,
        AgentSpec {
            role: "worker",
            tunnel_ip: Some("127.0.0.1"),
            age_minutes: 0,
            cordoned: false,
        },
    )
    .await;
    insert_node(&harness, Some(agent_id)).await;
    garage_stub().await;

    let (status, forwarded) = fetch(&harness, &format!("/api/s3/{external}/dir/object?x=1")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(forwarded["target"], format!("/{external}/dir/object?x=1"));
    let (status, forwarded) = fetch(&harness, &format!("/s3data/{internal}/object")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(forwarded["target"], format!("/{internal}/object"));
}
