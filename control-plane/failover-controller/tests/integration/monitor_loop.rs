use std::time::Duration;

use entity::entities::agents;
use failover_controller::db::events;
use failover_controller::services::monitor;
use sea_orm::{DatabaseConnection, EntityTrait};
use shared::leader::LeaderLease;
use uuid::Uuid;

use crate::common::{
    database, insert_agent, insert_metric, insert_workload, mock_url, recorded_calls, WorkloadSpec,
    STALE_LOCK,
};

async fn elected_leader() -> LeaderLease {
    let endpoints = std::env::var("ETCD_ENDPOINTS").expect("ETCD_ENDPOINTS must be set");
    let etcd = etcd_client::Client::connect(endpoints.split(',').collect::<Vec<_>>(), None)
        .await
        .expect("etcd must be reachable");
    let key = format!("/csfx/test/failover-controller/{}", Uuid::new_v4());
    let leader = LeaderLease::new(etcd, key, Uuid::new_v4().to_string()).spawn();
    for _ in 0..100 {
        if leader.is_leader() {
            return leader;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("leadership was not acquired");
}

async fn status_of(db: &DatabaseConnection, id: Uuid) -> String {
    agents::Entity::find_by_id(id)
        .one(db)
        .await
        .unwrap()
        .unwrap()
        .status
}

async fn settled(db: &DatabaseConnection, offline: Uuid, pressured: Uuid) -> bool {
    let failed_over = events::find_open(db, offline, "failover_complete")
        .await
        .unwrap()
        .is_some();
    failed_over && !recorded_calls(pressured).is_empty()
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL and etcd via ETCD_ENDPOINTS"]
async fn monitor_degrades_fails_over_and_relieves_pressure() {
    let _guard = STALE_LOCK.lock().await;
    mock_url();
    let db = database().await;
    let stale = insert_agent(&db, "Online", Some(100), None).await;
    let healthy = insert_agent(&db, "Online", Some(0), None).await;
    let dead = insert_agent(&db, "Degraded", Some(1000), None).await;
    insert_workload(&db, dead.id, WorkloadSpec::default()).await;
    let pressured = insert_agent(&db, "Online", Some(0), None).await;
    insert_workload(&db, pressured.id, WorkloadSpec::default()).await;
    insert_metric(&db, pressured.id, (10.0, 95.0, 10.0), 5).await;

    let leader = elected_leader().await;
    let task = tokio::spawn(monitor::run(db.clone(), leader));
    for _ in 0..200 {
        if settled(&db, dead.id, pressured.id).await {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    task.abort();

    assert_eq!(status_of(&db, stale.id).await, "Degraded");
    assert_eq!(status_of(&db, healthy.id).await, "Online");
    assert_eq!(status_of(&db, dead.id).await, "Offline");
    assert!(settled(&db, dead.id, pressured.id).await);
    let alert = events::find_open(&db, pressured.id, "high_memory")
        .await
        .unwrap();
    assert!(alert.is_some());
}
