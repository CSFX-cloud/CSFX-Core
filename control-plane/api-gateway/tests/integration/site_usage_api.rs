use axum::http::{Method, StatusCode};
use chrono::{Duration, Utc};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::common::Harness;
use crate::support_b::{full_harness, resource_group};

async fn site_with_usage(harness: &Harness, group: Uuid) -> String {
    let body = json!({"name": format!("u-{}", &Uuid::new_v4().simple().to_string()[..12]), "repo_url": "https://git.test/org/usage"});
    let (status, site) = harness
        .post(&format!("/api/resource-groups/{group}/sites"), body)
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let id: Uuid = site["id"].as_str().unwrap().parse().unwrap();
    let now = Utc::now().naive_utc();
    for (hours_ago, requests) in [(1, 10), (2, 5), (24 * 30, 99)] {
        entity::site_usage_hourly::ActiveModel {
            site_id: Set(id),
            hour: Set(now - Duration::hours(hours_ago)),
            requests: Set(requests),
            bytes_out: Set(requests * 100),
        }
        .insert(&harness.db)
        .await
        .unwrap();
    }
    format!("/api/resource-groups/{group}/sites/{id}/usage")
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn usage_sums_the_requested_window() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let uri = site_with_usage(&harness, group).await;

    let (status, usage) = harness.get(&uri).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(usage["days"], 7);
    assert_eq!(usage["requests"], 15);
    assert_eq!(usage["bytes_out"], 1500);
    assert!(usage["build_minutes"].is_number());

    let (_, wide) = harness.get(&format!("{uri}?days=90")).await;
    assert_eq!(wide["requests"], 114);
    let (_, clamped) = harness.get(&format!("{uri}?days=0")).await;
    assert_eq!(clamped["days"], 1);
    let (_, capped) = harness.get(&format!("{uri}?days=999")).await;
    assert_eq!(capped["days"], 90);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn usage_requires_permission_and_a_known_site() {
    let harness = full_harness().await;
    let group = resource_group(&harness).await;
    let uri = site_with_usage(&harness, group).await;
    let limited = harness.limited_token().await;

    assert_eq!(
        harness.send(Method::GET, &uri, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        harness
            .send(Method::GET, &uri, Some(&limited), None)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let unknown = format!(
        "/api/resource-groups/{group}/sites/{}/usage",
        Uuid::new_v4()
    );
    let (status, body): (StatusCode, Value) = harness.get(&unknown).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not found");
}
