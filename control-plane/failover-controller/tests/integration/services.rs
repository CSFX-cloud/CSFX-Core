use failover_controller::db::events;
use failover_controller::services::eviction::{pick_victim, Pressure};
use failover_controller::services::{hardware_alerts, status_alerts};
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    database, eviction_service, failover_service, insert_agent, insert_metric, insert_workload,
    recorded_calls, set_plan, Plan, WorkloadSpec,
};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn failover_detaches_reschedules_and_records_event() {
    let db = database().await;
    let agent = insert_agent(&db, "Offline", Some(1000), None).await;
    let workload = insert_workload(&db, agent.id, WorkloadSpec::default()).await;
    failover_service(db.clone())
        .handle_failover(agent.id)
        .await
        .unwrap();

    let calls = recorded_calls(agent.id);
    assert_eq!(calls.len(), 2);
    assert!(calls[0].starts_with("detach-all"));
    assert!(calls[1].starts_with("reschedule"));
    let event = events::find_open(&db, agent.id, "failover_complete")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(event.affected_workloads, Some(json!([workload.id])));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn failover_fails_when_detach_is_rejected() {
    let db = database().await;
    let agent = insert_agent(&db, "Offline", Some(1000), None).await;
    set_plan(
        agent.id,
        Plan {
            detach: 500,
            ..Plan::default()
        },
    );
    let error = failover_service(db.clone())
        .handle_failover(agent.id)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("volume detach request returned error"));
    assert_eq!(recorded_calls(agent.id).len(), 1);
    assert!(events::find_open(&db, agent.id, "failover_complete")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn failover_fails_when_reschedule_is_rejected() {
    let db = database().await;
    let agent = insert_agent(&db, "Offline", Some(1000), None).await;
    set_plan(
        agent.id,
        Plan {
            reschedule: 503,
            ..Plan::default()
        },
    );
    let error = failover_service(db.clone())
        .handle_failover(agent.id)
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("reschedule request returned error"));
    assert!(events::find_open(&db, agent.id, "failover_complete")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn eviction_moves_a_victim_once_per_cooldown() {
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    insert_workload(&db, agent.id, WorkloadSpec::default()).await;
    let service = eviction_service(db.clone());
    service.relieve(agent.id, Pressure::Memory).await.unwrap();
    service.relieve(agent.id, Pressure::Cpu).await.unwrap();
    assert_eq!(recorded_calls(agent.id).len(), 1);
    let event = events::find_open(&db, agent.id, "eviction").await.unwrap();
    assert!(event.is_some());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn eviction_without_victim_or_move_records_nothing() {
    let db = database().await;
    let service = eviction_service(db.clone());

    let empty = insert_agent(&db, "Online", Some(0), None).await;
    service.relieve(empty.id, Pressure::Cpu).await.unwrap();
    assert!(recorded_calls(empty.id).is_empty());

    let refused = insert_agent(&db, "Online", Some(0), None).await;
    insert_workload(&db, refused.id, WorkloadSpec::default()).await;
    let body = json!([{"status": "failed"}]);
    set_plan(
        refused.id,
        Plan {
            reschedule_body: body,
            ..Plan::default()
        },
    );
    service.relieve(refused.id, Pressure::Cpu).await.unwrap();
    assert_eq!(recorded_calls(refused.id).len(), 1);
    assert!(events::find_open(&db, refused.id, "eviction")
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn eviction_propagates_scheduler_errors() {
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    insert_workload(&db, agent.id, WorkloadSpec::default()).await;
    set_plan(
        agent.id,
        Plan {
            reschedule: 500,
            ..Plan::default()
        },
    );
    let result = eviction_service(db)
        .relieve(agent.id, Pressure::Memory)
        .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("eviction request rejected"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn victim_selection_ignores_non_running_workloads() {
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    let pending = WorkloadSpec {
        status: "scheduled",
        ..Default::default()
    };
    let first = insert_workload(&db, agent.id, pending).await;
    assert_eq!(pick_victim(std::slice::from_ref(&first), Pressure::Cpu), None);
    let second = insert_workload(&db, agent.id, WorkloadSpec::default()).await;
    assert_eq!(
        pick_victim(&[first, second.clone()], Pressure::Cpu),
        Some(second.id)
    );
}

async fn open_types(db: &sea_orm::DatabaseConnection, agent_id: Uuid) -> Vec<&'static str> {
    let mut found = Vec::new();
    for event_type in ["high_cpu", "high_memory", "high_disk"] {
        if events::find_open(db, agent_id, event_type)
            .await
            .unwrap()
            .is_some()
        {
            found.push(event_type);
        }
    }
    found
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn hardware_alerts_open_once_and_resolve_on_recovery() {
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    hardware_alerts::check_agent(&db, &agent).await;
    assert!(open_types(&db, agent.id).await.is_empty());

    insert_metric(&db, agent.id, (95.0, 95.0, 50.0), 20).await;
    insert_metric(&db, agent.id, (96.0, 99.0, 50.0), 10).await;
    hardware_alerts::check_agent(&db, &agent).await;
    hardware_alerts::check_agent(&db, &agent).await;
    assert_eq!(
        open_types(&db, agent.id).await,
        vec!["high_cpu", "high_memory"]
    );
    let cpu = events::find_open(&db, agent.id, "high_cpu")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(cpu.message.as_deref(), Some("CPU usage above 90% for 5m"));

    insert_metric(&db, agent.id, (10.0, 10.0, 10.0), 1).await;
    hardware_alerts::check_agent(&db, &agent).await;
    assert!(open_types(&db, agent.id).await.is_empty());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn status_alerts_follow_agent_status() {
    let db = database().await;
    let mut agent = insert_agent(&db, "Offline", Some(400), None).await;
    status_alerts::check_agent(&db, &agent).await;
    status_alerts::check_agent(&db, &agent).await;
    let open = events::find_open(&db, agent.id, "offline")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(open.severity, "critical");
    assert!(open.message.unwrap().contains("no heartbeat for"));

    agent.status = "Degraded".to_string();
    agent.last_heartbeat = None;
    status_alerts::check_agent(&db, &agent).await;
    assert!(events::find_open(&db, agent.id, "offline")
        .await
        .unwrap()
        .is_none());
    let degraded = events::find_open(&db, agent.id, "degraded")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        degraded.message,
        Some(format!("Node {} is degraded", agent.name))
    );

    agent.status = "Online".to_string();
    status_alerts::check_agent(&db, &agent).await;
    assert!(events::find_open(&db, agent.id, "degraded")
        .await
        .unwrap()
        .is_none());
    assert!(events::find_open(&db, agent.id, "online")
        .await
        .unwrap()
        .is_none());
    let recent = events::get_recent(&db, 100).await.unwrap();
    assert!(recent.iter().any(|event| event.agent_id == Some(agent.id)
        && event.event_type == "online"
        && event.status == "resolved"));
}
