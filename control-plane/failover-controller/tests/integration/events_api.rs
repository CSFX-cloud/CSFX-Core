use axum::http::{Method, StatusCode};
use failover_controller::db::events;
use serde_json::json;
use uuid::Uuid;

use chrono::{Duration, Utc};
use entity::entities::failover_events;
use sea_orm::{ActiveModelTrait, DatabaseConnection, EntityTrait, Set};

use crate::common::{call, call_page, call_text, insert_agent, router};

async fn insert_event_at(db: &DatabaseConnection, agent_id: Uuid, status: &str, age_secs: i64) {
    failover_events::ActiveModel {
        id: Set(Uuid::new_v4()),
        agent_id: Set(Some(agent_id)),
        event_type: Set("offline".to_string()),
        affected_workloads: Set(None),
        duration_ms: Set(None),
        created_at: Set(Utc::now().naive_utc() - Duration::seconds(age_secs)),
        severity: Set("warning".to_string()),
        status: Set(status.to_string()),
        resolved_at: Set(None),
        message: Set(None),
    }
    .insert(db)
    .await
    .unwrap();
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn health_and_metrics_respond() {
    let (app, _) = router().await;
    failover_controller::metrics::init();
    failover_controller::metrics::record_request("GET", "/health", 200, 0.01);
    let (status, body) = call_text(&app, Method::GET, "/health", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, "Failover Controller OK");
    let (status, body) = call_text(&app, Method::GET, "/metrics", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body.contains("csfx_http_requests_total"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn list_events_filters_by_agent_and_status() {
    let (app, db) = router().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    let first = events::insert(&db, Some(agent.id), "failover_complete", None, Some(5))
        .await
        .unwrap();
    events::insert_with_severity(&db, Some(agent.id), "offline", "critical", None, None, None)
        .await
        .unwrap();
    events::resolve(&db, first.id).await.unwrap();

    let uri = format!("/events?agent_id={}", agent.id);
    let (status, body) = call(&app, Method::GET, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 2);

    let uri = format!("/events?agent_id={}&status=open", agent.id);
    let (_, body) = call(&app, Method::GET, &uri, None).await;
    let open = body.as_array().unwrap();
    assert_eq!(open.len(), 1);
    assert_eq!(open[0]["event_type"], "offline");

    let uri = format!("/events?agent_id={}&status=resolved", agent.id);
    let (_, body) = call(&app, Method::GET, &uri, None).await;
    assert_eq!(body[0]["id"], json!(first.id));
    assert!(body[0]["resolved_at"].is_string());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn resolve_event_succeeds_and_rejects_unknown() {
    let (app, db) = router().await;
    let event = events::insert(&db, None, "failover_complete", None, None)
        .await
        .unwrap();
    let uri = format!("/events/{}/resolve", event.id);
    let (status, _) = call(&app, Method::POST, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let uri = format!("/events/{}/resolve", Uuid::new_v4());
    let (status, body) = call(&app, Method::POST, &uri, None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("Event not found"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn maintenance_can_be_set_and_cleared() {
    let (app, db) = router().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    let uri = format!("/agents/{}/maintenance", agent.id);

    let (status, body) = call(&app, Method::POST, &uri, Some(json!({"minutes": 0}))).await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["maintenance_until"].is_string());
    let alerting = failover_controller::db::agents::get_agents_for_alerting(&db)
        .await
        .unwrap();
    assert!(alerting.iter().all(|candidate| candidate.id != agent.id));

    let (status, _) = call(&app, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let alerting = failover_controller::db::agents::get_agents_for_alerting(&db)
        .await
        .unwrap();
    assert!(alerting.iter().any(|candidate| candidate.id == agent.id));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn maintenance_rejects_unknown_agent() {
    let (app, _) = router().await;
    let uri = format!("/agents/{}/maintenance", Uuid::new_v4());
    let (status, body) = call(&app, Method::POST, &uri, Some(json!({"minutes": 5}))).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(body["error"].as_str().unwrap().contains("Agent not found"));
    let (status, _) = call(&app, Method::DELETE, &uri, None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn open_filter_is_applied_before_the_limit() {
    let (app, db) = router().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    insert_event_at(&db, agent.id, "open", 10_000).await;
    let resolved: Vec<_> = (0..150)
        .map(|age| failover_events::ActiveModel {
            id: Set(Uuid::new_v4()),
            agent_id: Set(Some(agent.id)),
            event_type: Set("offline".to_string()),
            affected_workloads: Set(None),
            duration_ms: Set(None),
            created_at: Set(Utc::now().naive_utc() - Duration::seconds(age)),
            severity: Set("warning".to_string()),
            status: Set("resolved".to_string()),
            resolved_at: Set(None),
            message: Set(None),
        })
        .collect();
    failover_events::Entity::insert_many(resolved)
        .exec(&db)
        .await
        .unwrap();

    let uri = format!("/events?agent_id={}&status=open", agent.id);
    let (status, body) = call(&app, Method::GET, &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn events_page_with_cursor_without_gaps() {
    let (app, db) = router().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    for age in 0..7 {
        insert_event_at(&db, agent.id, "open", age).await;
    }

    let mut seen = Vec::new();
    let mut uri = format!("/events?agent_id={}&limit=3", agent.id);
    loop {
        let (status, body, cursor) = call_page(&app, &uri).await;
        assert_eq!(status, StatusCode::OK);
        seen.extend(body.as_array().unwrap().iter().map(|e| e["id"].clone()));
        let Some(cursor) = cursor else { break };
        uri = format!("/events?agent_id={}&limit=3&cursor={}", agent.id, cursor);
    }
    let unique: std::collections::HashSet<_> = seen.iter().map(|v| v.to_string()).collect();
    assert_eq!(seen.len(), 7);
    assert_eq!(unique.len(), 7);

    let uri = format!("/events?agent_id={}&cursor=garbage", agent.id);
    let (status, _, _) = call_page(&app, &uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
