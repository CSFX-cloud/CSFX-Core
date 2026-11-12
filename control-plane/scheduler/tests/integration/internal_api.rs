use axum::http::{Method, StatusCode};
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    call, create_pending_workload, find_workload, insert_agent, migrated_database, router,
};

async fn assigned_workload(app: &axum::Router) -> (String, Uuid) {
    let id = create_pending_workload(app).await;
    let agent = insert_agent(&migrated_database().await).await;
    scheduler::db::workloads::assign(&migrated_database().await, id.parse().unwrap(), agent)
        .await
        .unwrap();
    (id, agent)
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn container_status_updates_are_stored() {
    let app = router().await;
    let (id, agent) = assigned_workload(&app).await;

    let (status, _) = call(
        &app,
        Method::POST,
        "/internal/workloads/status",
        Some(json!({"agent_id": agent, "statuses": [
            {"workload_id": id, "container_id": "abc123", "status": "running"},
            {"workload_id": Uuid::new_v4(), "container_id": "", "status": "failed"}
        ]})),
    )
    .await;

    assert_eq!(status, StatusCode::NO_CONTENT);
    let workload = find_workload(&app, &id).await;
    assert_eq!(workload["status"], "running");
    assert_eq!(workload["container_id"], "abc123");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn stats_updates_are_stored() {
    let app = router().await;
    let (id, agent) = assigned_workload(&app).await;
    let sample = |cpu: f64, memory: i64| {
        json!({"agent_id": agent, "stats": [
            {
                "workload_id": id,
                "cpu_usage_percent": cpu,
                "memory_usage_bytes": memory,
                "network_rx_bytes": 10,
                "network_tx_bytes": 20
            },
            {"workload_id": Uuid::new_v4(), "cpu_usage_percent": 1.0}
        ]})
    };

    for (cpu, memory) in [(80.0, 4096), (20.0, 1024)] {
        let (status, _) = call(
            &app,
            Method::POST,
            "/internal/workloads/stats",
            Some(sample(cpu, memory)),
        )
        .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
    }

    let workload = find_workload(&app, &id).await;
    assert_eq!(workload["cpu_usage_percent"], 20.0);
    assert_eq!(workload["memory_usage_bytes"], 1024);
    assert_eq!(workload["network_rx_bytes"], 10);
    assert!(workload["recommended_cpu_millicores"].is_i64());
    assert!(workload["recommended_memory_bytes"].is_i64());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn restart_acknowledgement_clears_request() {
    let app = router().await;
    let (id, agent) = assigned_workload(&app).await;
    call(
        &app,
        Method::POST,
        &format!("/workloads/{}/restart", id),
        None,
    )
    .await;

    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/internal/workloads/{}/restart-ack", id),
        Some(json!({"agent_id": agent})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/internal/workloads/{}/restart-ack", Uuid::new_v4()),
        Some(json!({"agent_id": agent})),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn foreign_agents_cannot_change_workloads() {
    let app = router().await;
    let (id, _) = assigned_workload(&app).await;
    let intruder = insert_agent(&migrated_database().await).await;
    let before = find_workload(&app, &id).await;

    let (status, _) = call(
        &app,
        Method::POST,
        "/internal/workloads/status",
        Some(json!({"agent_id": intruder, "statuses": [
            {"workload_id": id, "container_id": "evil", "status": "running"}
        ]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(
        &app,
        Method::POST,
        "/internal/workloads/stats",
        Some(json!({"agent_id": intruder, "stats": [
            {"workload_id": id, "cpu_usage_percent": 99.0}
        ]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    call(
        &app,
        Method::POST,
        &format!("/workloads/{}/restart", id),
        None,
    )
    .await;
    let (status, _) = call(
        &app,
        Method::POST,
        &format!("/internal/workloads/{}/restart-ack", id),
        Some(json!({"agent_id": intruder})),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let after = find_workload(&app, &id).await;
    assert_eq!(after["status"], before["status"]);
    assert_eq!(after["container_id"], before["container_id"]);
    assert_eq!(after["cpu_usage_percent"], before["cpu_usage_percent"]);
    let stored =
        scheduler::db::workloads::get_by_id(&migrated_database().await, id.parse().unwrap())
            .await
            .unwrap()
            .unwrap();
    assert!(stored.restart_requested);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_statuses_are_ignored() {
    let app = router().await;
    let (id, agent) = assigned_workload(&app).await;
    let before = find_workload(&app, &id).await;

    let (status, _) = call(
        &app,
        Method::POST,
        "/internal/workloads/status",
        Some(json!({"agent_id": agent, "statuses": [
            {"workload_id": id, "container_id": "abc", "status": "definitely-not-a-status"}
        ]})),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(find_workload(&app, &id).await["status"], before["status"]);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reschedule_reports_pending_when_nothing_fits() {
    let app = router().await;
    let id = create_pending_workload(&app).await;
    let dead_agent = Uuid::new_v4();

    let (status, results) = call(
        &app,
        Method::POST,
        &format!("/internal/agents/{}/reschedule", dead_agent),
        Some(json!({"workload_ids": [id]})),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(results[0]["status"], "pending");
    assert_eq!(results[0]["workload_id"], id);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reschedule_rejects_unknown_workload() {
    let app = router().await;

    let (status, error) = call(
        &app,
        Method::POST,
        &format!("/internal/agents/{}/reschedule", Uuid::new_v4()),
        Some(json!({"workload_ids": [Uuid::new_v4()]})),
    )
    .await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert!(error["error"].as_str().unwrap().contains("not found"));
}
