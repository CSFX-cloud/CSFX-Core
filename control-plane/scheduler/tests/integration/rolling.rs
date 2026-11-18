use std::time::Duration;

use chrono::Utc;
use entity::entities::workloads as workload_rows;
use scheduler::db::workloads;
use scheduler::models::workload::UpdateWorkloadRequest;
use scheduler::services::scheduler::SchedulerService;
use sea_orm::sea_query::Expr;
use sea_orm::{ColumnTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::common::{placing, small_request, unique_name};

fn image_update(image: &str) -> UpdateWorkloadRequest {
    UpdateWorkloadRequest {
        image: Some(image.to_string()),
        env_vars: None,
        ports: None,
        restart_policy: None,
        max_restarts: None,
        volume_mounts: None,
    }
}

async fn running_workload(service: &SchedulerService, db: &DatabaseConnection) -> Uuid {
    let response = service
        .schedule(small_request(&unique_name("surge")))
        .await
        .unwrap();
    set_status(db, response.workload_id, "running").await;
    response.workload_id
}

async fn set_status(db: &DatabaseConnection, id: Uuid, status: &str) {
    workloads::update_container_status(db, id, "container", status)
        .await
        .unwrap();
}

async fn fetch(db: &DatabaseConnection, id: Uuid) -> Option<workload_rows::Model> {
    workloads::get_by_id(db, id).await.unwrap()
}

#[tokio::test]
#[ignore = "requires postgres"]
async fn an_update_runs_next_to_the_old_instance_and_then_takes_over() {
    let fixture = placing().await;
    let old_id = running_workload(&fixture.service, &fixture.db).await;
    let old = fetch(&fixture.db, old_id).await.unwrap();

    let returned = fixture
        .service
        .apply_update(old_id, &image_update("nginx:2"))
        .await
        .unwrap();
    assert_eq!(returned.id, old_id);
    assert_eq!(returned.image, "nginx:latest");

    let replacement = workloads::replacement_for(&fixture.db, old_id)
        .await
        .unwrap()
        .expect("a replacement must exist");
    assert_eq!(replacement.image, "nginx:2");
    assert_eq!(replacement.name, old.name);
    assert!(replacement.service_name.is_none());
    set_status(&fixture.db, replacement.id, "running").await;

    fixture.service.reconcile_replacements().await.unwrap();
    assert!(fetch(&fixture.db, replacement.id)
        .await
        .unwrap()
        .replaces_workload_id
        .is_some());
    tokio::time::sleep(Duration::from_secs(11)).await;
    set_status(&fixture.db, replacement.id, "running").await;
    fixture.service.reconcile_replacements().await.unwrap();

    let taken_over = fetch(&fixture.db, replacement.id).await.unwrap();
    assert!(taken_over.replaces_workload_id.is_none());
    let retiring = fetch(&fixture.db, old_id).await.unwrap();
    assert!(retiring.retire_after.is_some());

    workload_rows::Entity::update_many()
        .col_expr(
            workload_rows::Column::RetireAfter,
            Expr::value(Utc::now().naive_utc() - chrono::Duration::seconds(1)),
        )
        .filter(workload_rows::Column::Id.eq(old_id))
        .exec(&fixture.db)
        .await
        .unwrap();
    fixture.service.reconcile_replacements().await.unwrap();
    assert!(fetch(&fixture.db, old_id).await.is_none());
}

#[tokio::test]
#[ignore = "requires postgres"]
async fn a_failed_replacement_is_removed_and_reported_on_the_old_workload() {
    let fixture = placing().await;
    let old_id = running_workload(&fixture.service, &fixture.db).await;
    fixture
        .service
        .apply_update(old_id, &image_update("nginx:broken"))
        .await
        .unwrap();
    let replacement = workloads::replacement_for(&fixture.db, old_id)
        .await
        .unwrap()
        .unwrap();
    set_status(&fixture.db, replacement.id, "failed").await;

    fixture.service.reconcile_replacements().await.unwrap();

    assert!(fetch(&fixture.db, replacement.id).await.is_none());
    let kept = fetch(&fixture.db, old_id).await.unwrap();
    assert!(kept.rollout_error.is_some());
    assert_eq!(kept.image, "nginx:latest");
}

#[tokio::test]
#[ignore = "requires postgres"]
async fn a_second_update_during_a_rollout_conflicts() {
    let fixture = placing().await;
    let old_id = running_workload(&fixture.service, &fixture.db).await;
    fixture
        .service
        .apply_update(old_id, &image_update("nginx:2"))
        .await
        .unwrap();

    let update = fixture
        .service
        .apply_update(old_id, &image_update("nginx:3"))
        .await;
    let restart = fixture.service.apply_restart(old_id).await;

    assert!(matches!(update, Err(DbErr::RecordNotUpdated)));
    assert!(matches!(restart, Err(DbErr::RecordNotUpdated)));
}
