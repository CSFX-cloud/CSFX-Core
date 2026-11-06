use axum::http::{Method, StatusCode};
use chrono::{Duration, SecondsFormat, Utc};
use serde_json::Value;
use uuid::Uuid;

use crate::common::{harness, Harness};
use crate::support_a::{insert_log, raw, unique, LogSpec};

fn stamp(offset: Duration) -> String {
    (Utc::now() + offset).to_rfc3339_opts(SecondsFormat::Secs, true)
}

async fn seed(harness: &Harness) -> String {
    let service = unique("svc");
    let rows = [
        ("ERROR", "system", "disk failure", None),
        ("WARN", "system", "slow response", None),
        ("INFO", "audit", "user created workload", Some("create")),
    ];
    for (level, kind, message, action) in rows {
        let spec = LogSpec {
            service: &service,
            level,
            kind,
            message,
            action,
        };
        insert_log(&harness.db, spec).await;
    }
    service
}

fn entries(body: &Value) -> Vec<&Value> {
    body["entries"].as_array().unwrap().iter().collect()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn log_routes_require_the_view_permission() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    for uri in [
        "/api/logs",
        "/api/logs/export",
        "/api/logs/facets",
        "/api/logs/trend",
    ] {
        let (status, _) = harness.send(Method::GET, uri, Some(&limited), None).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{uri}");
        let (status, _) = harness.send(Method::GET, uri, None, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{uri}");
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn logs_are_filtered_and_paginated() {
    let harness = harness().await;
    let service = seed(&harness).await;
    let base = format!("/api/logs?service={service}");

    let (status, all) = harness.get(&base).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(all["total"], 3);
    assert_eq!(all["has_more"], false);
    assert!(entries(&all)[0]["cef"]
        .as_str()
        .unwrap()
        .starts_with("CEF:"));

    let (_, page) = harness.get(&format!("{base}&limit=2")).await;
    assert_eq!(entries(&page).len(), 2);
    assert_eq!(page["has_more"], true);
    let (_, tail) = harness.get(&format!("{base}&limit=2&offset=2")).await;
    assert_eq!(entries(&tail).len(), 1);

    let (_, levels) = harness.get(&format!("{base}&level=ERROR,%20WARN")).await;
    assert_eq!(levels["total"], 2);
    let (_, audit) = harness
        .get(&format!("{base}&kind=audit&action=create"))
        .await;
    assert_eq!(audit["total"], 1);
    let (_, typed) = harness
        .get(&format!(
            "{base}&target_type=workload&classification=system"
        ))
        .await;
    assert_eq!(typed["total"], 1);
    let (_, search) = harness.get(&format!("{base}&q=disk")).await;
    assert_eq!(search["total"], 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn log_filters_by_identity_and_time_window() {
    let harness = harness().await;
    let service = seed(&harness).await;
    let base = format!("/api/logs?service={service}");
    let random = Uuid::new_v4();

    for filter in [
        format!("agent_id={random}"),
        format!("workload_id={random}"),
        format!("organization_id={random}"),
        format!("actor_user_id={random}"),
        "target_id=missing".to_string(),
    ] {
        let (_, none) = harness.get(&format!("{base}&{filter}")).await;
        assert_eq!(none["total"], 0, "{filter}");
    }
    let (_, inside) = harness
        .get(&format!(
            "{base}&from={}&to={}",
            stamp(Duration::hours(-1)),
            stamp(Duration::hours(1))
        ))
        .await;
    assert_eq!(inside["total"], 3);
    let (_, past) = harness
        .get(&format!("{base}&to={}", stamp(Duration::hours(-1))))
        .await;
    assert_eq!(past["total"], 0);
    let (_, limited) = harness.get(&format!("{base}&limit=100000")).await;
    assert_eq!(limited["total"], 3);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn facets_count_the_matching_rows() {
    let harness = harness().await;
    let service = seed(&harness).await;
    let (status, facets) = harness
        .get(&format!("/api/logs/facets?service={service}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let count = |group: &str, value: &str| {
        facets[group]
            .as_array()
            .unwrap()
            .iter()
            .find(|facet| facet["value"] == value)
            .map(|facet| facet["count"].as_i64().unwrap())
    };
    assert_eq!(count("kinds", "system"), Some(2));
    assert_eq!(count("kinds", "audit"), Some(1));
    assert_eq!(count("classifications", "system"), Some(3));
    assert_eq!(count("target_types", "workload"), Some(1));
    assert_eq!(count("actions", "create"), Some(1));
    assert!(facets["services"]
        .as_array()
        .unwrap()
        .iter()
        .any(|f| f["value"] == service));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn trend_buckets_cover_the_window() {
    let harness = harness().await;
    let service = seed(&harness).await;
    let (status, points) = harness
        .get(&format!("/api/logs/trend?service={service}"))
        .await;
    assert_eq!(status, StatusCode::OK);
    let points = points.as_array().unwrap();
    assert_eq!(points.len(), 60);
    let total: i64 = points
        .iter()
        .map(|point| point["count"].as_i64().unwrap())
        .sum();
    assert_eq!(total, 3);

    let window = format!(
        "/api/logs/trend?service={service}&from={}&to={}",
        stamp(Duration::minutes(-30)),
        stamp(Duration::minutes(30))
    );
    let (_, narrow) = harness.get(&window).await;
    assert_eq!(narrow.as_array().unwrap().len(), 60);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn export_returns_cef_lines() {
    let harness = harness().await;
    let service = seed(&harness).await;
    let (status, headers, body) = raw(
        &harness,
        Method::GET,
        &format!("/api/logs/export?service={service}"),
        &[("authorization", &format!("Bearer {}", harness.token))],
        Vec::new(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(headers["content-disposition"]
        .to_str()
        .unwrap()
        .contains("csfx-logs.cef"));
    let text = String::from_utf8(body.to_vec()).unwrap();
    assert_eq!(text.lines().count(), 3);
    assert!(text.lines().all(|line| line.starts_with("CEF:")));
}
