use failover_controller::db::{agents, events};
use uuid::Uuid;

use crate::common::{ago, database, insert_agent, insert_workload, WorkloadSpec, STALE_LOCK};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn stale_agents_split_into_degraded_and_offline() {
    let _guard = STALE_LOCK.lock().await;
    let db = database().await;
    let online_stale = insert_agent(&db, "Online", Some(100), None).await;
    let online_fresh = insert_agent(&db, "Online", Some(1), None).await;
    let degraded_old = insert_agent(&db, "Degraded", Some(1000), None).await;
    let offline_old = insert_agent(&db, "Offline", Some(1000), None).await;
    let online_ancient = insert_agent(&db, "Online", Some(1000), None).await;
    let no_heartbeat = insert_agent(&db, "Online", None, None).await;
    let in_maintenance = insert_agent(&db, "Online", Some(1000), Some(ago(-600))).await;

    let (degraded, offline) = agents::get_stale_agents(&db, 50, 300).await.unwrap();
    let degraded: Vec<Uuid> = degraded.iter().map(|agent| agent.id).collect();
    let offline: Vec<Uuid> = offline.iter().map(|agent| agent.id).collect();

    assert!(degraded.contains(&online_stale.id));
    assert!(degraded.contains(&online_ancient.id));
    assert!(offline.contains(&degraded_old.id));
    assert!(offline.contains(&offline_old.id));
    for ignored in [online_fresh.id, no_heartbeat.id, in_maintenance.id] {
        assert!(!degraded.contains(&ignored) && !offline.contains(&ignored));
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn set_agent_status_updates_and_rejects_unknown() {
    let _guard = STALE_LOCK.lock().await;
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(1000), None).await;
    agents::set_agent_status(&db, agent.id, "Offline")
        .await
        .unwrap();
    let (_, offline) = agents::get_stale_agents(&db, 50, 300).await.unwrap();
    assert!(offline.iter().any(|candidate| candidate.id == agent.id));
    assert!(agents::set_agent_status(&db, Uuid::new_v4(), "Online")
        .await
        .is_err());
    assert!(agents::count_online(&db).await.unwrap() >= 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn active_assignments_follow_workload_status() {
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    assert!(!agents::has_active_assignments(&db, agent.id).await.unwrap());
    let spec = WorkloadSpec {
        status: "stopped",
        ..Default::default()
    };
    insert_workload(&db, agent.id, spec).await;
    assert!(!agents::has_active_assignments(&db, agent.id).await.unwrap());
    insert_workload(&db, agent.id, WorkloadSpec::default()).await;
    assert!(agents::has_active_assignments(&db, agent.id).await.unwrap());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn open_events_are_found_until_resolved() {
    let db = database().await;
    let agent = insert_agent(&db, "Online", Some(0), None).await;
    assert!(events::find_open(&db, agent.id, "offline")
        .await
        .unwrap()
        .is_none());
    let event = events::insert_with_severity(
        &db,
        Some(agent.id),
        "offline",
        "critical",
        Some("down".to_string()),
        Some(vec![Uuid::new_v4()]),
        Some(3),
    )
    .await
    .unwrap();
    let found = events::find_open(&db, agent.id, "offline").await.unwrap();
    assert_eq!(found.unwrap().id, event.id);
    events::resolve(&db, event.id).await.unwrap();
    assert!(events::find_open(&db, agent.id, "offline")
        .await
        .unwrap()
        .is_none());
    let recent = events::get_recent(&db, 100).await.unwrap();
    assert!(recent.len() <= 100);
}
