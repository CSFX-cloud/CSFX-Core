use api_gateway::prune::{prune_audit, prune_logs};
use axum::http::{Method, StatusCode};
use chrono::{Duration, SecondsFormat, Utc};
use entity::entities::{audit_log, logs};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter,
};
use serde_json::{json, Value};
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

fn audit_row(service: &str, age: Duration) -> audit_log::ActiveModel {
    let seen_at = (Utc::now() - age).fixed_offset();
    audit_log::ActiveModel {
        id: Set(Uuid::new_v4()),
        service: Set(service.to_string()),
        level: Set("INFO".to_string()),
        classification: Set("audit".to_string()),
        message: Set("workloads.create".to_string()),
        agent_id: Set(None),
        workload_id: Set(None),
        organization_id: Set(None),
        created_at: Set(seen_at),
        kind: Set("user".to_string()),
        actor_user_id: Set(None),
        action: Set(Some("workloads.create".to_string())),
        target_type: Set(Some("workloads".to_string())),
        target_id: Set(None),
        fields: Set(json!({})),
        occurrences: Set(1),
        last_seen_at: Set(seen_at),
    }
}

fn log_row(service: &str, age: Duration) -> logs::ActiveModel {
    let seen_at = (Utc::now() - age).fixed_offset();
    logs::ActiveModel {
        id: Set(Uuid::new_v4()),
        service: Set(service.to_string()),
        level: Set("INFO".to_string()),
        classification: Set("system".to_string()),
        message: Set("old entry".to_string()),
        agent_id: Set(None),
        workload_id: Set(None),
        organization_id: Set(None),
        created_at: Set(seen_at),
        kind: Set("system".to_string()),
        actor_user_id: Set(None),
        action: Set(None),
        target_type: Set(None),
        target_id: Set(None),
        fields: Set(json!({})),
        occurrences: Set(1),
        last_seen_at: Set(seen_at),
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn keyset_cursor_pages_through_all_rows_without_total() {
    let harness = harness().await;
    let service = unique("keyset");
    for minute in 0..5 {
        let mut row = log_row(&service, Duration::minutes(minute / 2));
        row.message = Set(format!("entry {minute}"));
        row.insert(&harness.db).await.unwrap();
    }
    let base = format!("/api/logs?service={service}&limit=2");

    let mut seen = Vec::new();
    let mut cursor = String::new();
    loop {
        let (status, page) = harness.get(&format!("{base}&cursor={cursor}")).await;
        assert_eq!(status, StatusCode::OK);
        assert!(page["total"].is_null());
        seen.extend(
            entries(&page)
                .iter()
                .map(|e| e["id"].as_str().unwrap().to_string()),
        );
        if !page["has_more"].as_bool().unwrap() {
            assert!(page["next_cursor"].is_null());
            break;
        }
        cursor = page["next_cursor"].as_str().unwrap().to_string();
    }

    let mut unique_ids = seen.clone();
    unique_ids.sort();
    unique_ids.dedup();
    assert_eq!(seen.len(), 5);
    assert_eq!(unique_ids.len(), 5);

    let (status, _) = harness.get(&format!("{base}&cursor=garbage")).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn user_kind_reads_from_the_audit_table() {
    let harness = harness().await;
    let service = unique("audit-read");
    audit_row(&service, Duration::minutes(1))
        .insert(&harness.db)
        .await
        .unwrap();

    let (_, audit) = harness
        .get(&format!("/api/logs?service={service}&kind=user"))
        .await;
    assert_eq!(audit["total"], 1);
    assert_eq!(entries(&audit)[0]["kind"], "user");
    let (_, system) = harness
        .get(&format!("/api/logs?service={service}&kind=system"))
        .await;
    assert_eq!(system["total"], 0);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn audit_retention_is_independent_from_log_prune() {
    let harness = harness().await;
    let service = unique("retention");
    let age = Duration::days(400);
    audit_row(&service, age).insert(&harness.db).await.unwrap();
    log_row(&service, age).insert(&harness.db).await.unwrap();
    let audit_count = || {
        audit_log::Entity::find()
            .filter(audit_log::Column::Service.eq(service.clone()))
            .count(&harness.db)
    };
    let log_count = || {
        logs::Entity::find()
            .filter(logs::Column::Service.eq(service.clone()))
            .count(&harness.db)
    };

    prune_logs(&harness.db, Utc::now() - Duration::days(30))
        .await
        .unwrap();
    assert_eq!(log_count().await.unwrap(), 0);
    assert_eq!(audit_count().await.unwrap(), 1);

    prune_audit(&harness.db, Utc::now() - Duration::days(365))
        .await
        .unwrap();
    assert_eq!(audit_count().await.unwrap(), 0);
}
