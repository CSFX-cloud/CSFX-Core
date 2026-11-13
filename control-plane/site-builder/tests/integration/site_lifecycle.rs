use std::time::Duration;

use axum::http::{Method, StatusCode};
use chrono::Utc;
use entity::ingress_routes;
use sea_orm::{ActiveModelTrait, ConnectionTrait, Set, Statement};
use serde_json::json;
use uuid::Uuid;

use crate::common::{
    call, deployment_of, harness, insert_deployment, insert_domain, insert_site, mark,
    mock_registry, point_site_at, relaxed, routes_for, site_of, tar_gz, SECRET,
};
use site_builder::artifacts;
use site_builder::job::builder_name;

async fn promote(app: &axum::Router, id: Uuid, body: Option<serde_json::Value>) -> StatusCode {
    let uri = format!("/internal/deployments/{id}/promote");
    call(app, Method::POST, &uri, Some(SECRET), body).await.0
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn promoting_requires_a_ready_deployment() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let building = insert_deployment(&harness.db, site.id, "building", "preview", "dev").await;
    assert_eq!(
        promote(&harness.app, building.id, None).await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        promote(&harness.app, Uuid::new_v4(), None).await,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn promoting_an_older_deployment_rolls_back_the_newer_one() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let older = insert_deployment(&harness.db, site.id, "ready", "production", "main").await;
    tokio::time::sleep(Duration::from_millis(10)).await;
    let newer = insert_deployment(&harness.db, site.id, "ready", "production", "main").await;
    point_site_at(&harness.db, site, newer.id).await;
    assert_eq!(promote(&harness.app, older.id, None).await, StatusCode::OK);
    assert_eq!(
        deployment_of(&harness.db, newer.id).await.status,
        "rolled_back"
    );
    let stored = deployment_of(&harness.db, older.id).await;
    assert_eq!(
        (stored.status.as_str(), stored.environment.as_str()),
        ("ready", "production")
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn container_promotion_needs_an_image_and_forwards_the_environment() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "container", Some(8080)).await;
    let imageless = insert_deployment(&harness.db, site.id, "ready", "preview", "dev").await;
    assert_eq!(
        promote(&harness.app, imageless.id, None).await,
        StatusCode::BAD_REQUEST
    );
    let ready = insert_deployment(&harness.db, site.id, "ready", "preview", "dev").await;
    let ready = mark(&harness.db, ready, |active| {
        active.image_ref = Set(Some("sites.csfx.invalid/x:1".into()))
    })
    .await;
    let body = json!({ "env": { "TOKEN": "abc" } });
    assert_eq!(
        promote(&harness.app, ready.id, Some(body)).await,
        StatusCode::OK
    );
    let created = harness.scheduler.created.lock().unwrap().clone();
    assert_eq!(created[0]["env_vars"]["TOKEN"], "abc");
    assert_eq!(created[0]["env_vars"]["PORT"], "8080");
    assert_eq!(
        site_of(&harness.db, site.id).await.current_deployment_id,
        Some(ready.id)
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn replacement_workloads_take_over_routes_and_retire_the_previous_one() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "container", Some(3000)).await;
    let hostname = insert_domain(&harness.db, site.id, "production", None).await;
    let old = insert_deployment(&harness.db, site.id, "ready", "production", "main").await;
    let old_workload = Uuid::new_v4();
    mark(&harness.db, old.clone(), |active| {
        active.workload_id = Set(Some(old_workload))
    })
    .await;
    point_site_at(&harness.db, site.clone(), old.id).await;
    let next = insert_deployment(&harness.db, site.id, "ready", "preview", "main").await;
    let next = mark(&harness.db, next, |active| {
        active.image_ref = Set(Some("sites.csfx.invalid/x:2".into()))
    })
    .await;
    assert_eq!(promote(&harness.app, next.id, None).await, StatusCode::OK);
    let replacement = deployment_of(&harness.db, next.id)
        .await
        .workload_id
        .unwrap();
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        if deployment_of(&harness.db, old.id)
            .await
            .workload_id
            .is_none()
        {
            break;
        }
    }
    assert!(harness
        .scheduler
        .deleted
        .lock()
        .unwrap()
        .contains(&old_workload));
    assert_eq!(deployment_of(&harness.db, old.id).await.workload_id, None);
    let routes = routes_for(&harness.db, &hostname).await;
    assert_eq!(
        routes
            .iter()
            .map(|route| route.workload_id)
            .collect::<Vec<_>>(),
        vec![replacement]
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reconcile_creates_and_removes_ingress_routes() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "container", Some(3000)).await;
    let hostname = insert_domain(&harness.db, site.id, "production", None).await;
    let workload = Uuid::new_v4();
    let deployment = insert_deployment(&harness.db, site.id, "ready", "production", "main").await;
    mark(&harness.db, deployment.clone(), |active| {
        active.workload_id = Set(Some(workload))
    })
    .await;
    point_site_at(&harness.db, site.clone(), deployment.id).await;
    let uri = format!("/internal/sites/{}/reconcile", site.id);
    assert_eq!(
        call(&harness.app, Method::POST, &uri, Some(SECRET), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(routes_for(&harness.db, &hostname).await.len(), 1);
    assert_eq!(
        call(&harness.app, Method::POST, &uri, Some(SECRET), None)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(routes_for(&harness.db, &hostname).await.len(), 1);
    let stale = relaxed(&harness.db).await;
    ingress_routes::ActiveModel {
        id: Set(Uuid::new_v4()),
        resource_group_id: Set(site.resource_group_id),
        workload_id: Set(workload),
        gateway_id: Set(None),
        hostname: Set(format!("old-{hostname}")),
        container_port: Set(3000),
        tls: Set(true),
        created_at: Set(Utc::now().naive_utc()),
    }
    .insert(&stale)
    .await
    .unwrap();
    stale.commit().await.unwrap();
    call(&harness.app, Method::POST, &uri, Some(SECRET), None).await;
    assert!(routes_for(&harness.db, &format!("old-{hostname}"))
        .await
        .is_empty());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reconcile_ignores_static_and_unknown_sites() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let uri = format!("/internal/sites/{}/reconcile", site.id);
    assert_eq!(
        call(&harness.app, Method::POST, &uri, Some(SECRET), None)
            .await
            .0,
        StatusCode::OK
    );
    let unknown = format!("/internal/sites/{}/reconcile", Uuid::new_v4());
    assert_eq!(
        call(&harness.app, Method::POST, &unknown, Some(SECRET), None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pruning_drops_the_payload_of_old_static_deployments() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let mut ids = Vec::new();
    for _ in 0..4 {
        let deployment = insert_deployment(&harness.db, site.id, "ready", "preview", "main").await;
        artifacts::store(&harness.db, deployment.id, tar_gz(&[("index.html", b"x")]))
            .await
            .unwrap();
        let key = deployment.id.to_string();
        mark(&harness.db, deployment.clone(), |active| {
            active.artifact_key = Set(Some(key))
        })
        .await;
        ids.push(deployment.id);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    harness.builds.prune_site(site.id).await;
    for (index, id) in ids.iter().enumerate() {
        let kept = deployment_of(&harness.db, *id).await.artifact_key.is_some();
        assert_eq!(kept, index >= 2, "deployment {index}");
        assert_eq!(
            artifacts::exists(&harness.db, *id).await.unwrap(),
            index >= 2
        );
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pruning_deletes_registry_tags_of_old_images() {
    let site = insert_site(
        &crate::common::migrated_database().await,
        "container",
        Some(3000),
    )
    .await;
    let name = site.id.to_string();
    let mock = mock_registry(&[(name.as_str(), &["aaaa0001", "aaaa0002", "aaaa0003"])]).await;
    let harness = harness(Some(&mock), None).await;
    for tag in ["aaaa0001", "aaaa0002", "aaaa0003"] {
        let deployment = insert_deployment(&harness.db, site.id, "ready", "preview", "main").await;
        let image = format!("sites.csfx.invalid/{name}:{tag}");
        mark(&harness.db, deployment, |active| {
            active.image_ref = Set(Some(image))
        })
        .await;
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    harness.builds.prune_site(site.id).await;
    assert_eq!(
        mock.state.repositories.lock().unwrap()[&name],
        vec!["aaaa0002", "aaaa0003"]
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn stale_builds_are_reaped_while_fresh_ones_survive() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let stale = insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    let past = (Utc::now() - chrono::Duration::hours(3)).naive_utc();
    mark(&harness.db, stale.clone(), |active| {
        active.created_at = Set(past)
    })
    .await;
    let fresh = insert_deployment(&harness.db, site.id, "building", "production", "main").await;
    harness.builds.reap_stale().await.unwrap();
    let reaped = deployment_of(&harness.db, stale.id).await;
    assert_eq!(reaped.status, "failed");
    assert!(reaped.build_log.contains("build timed out"));
    assert_eq!(
        deployment_of(&harness.db, fresh.id).await.status,
        "building"
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn reaping_a_queued_build_releases_its_unrecorded_builder() {
    let harness = harness(None, None).await;
    let site = insert_site(&harness.db, "static", None).await;
    let stale = insert_deployment(&harness.db, site.id, "queued", "production", "main").await;
    let past = (Utc::now() - chrono::Duration::hours(3)).naive_utc();
    mark(&harness.db, stale.clone(), |active| {
        active.created_at = Set(past)
    })
    .await;
    let builder = Uuid::new_v4();
    harness
        .db
        .execute(Statement::from_sql_and_values(
            harness.db.get_database_backend(),
            "INSERT INTO workloads (id, name, image, cpu_millicores, memory_bytes, disk_bytes, resource_group_id, created_at) VALUES ($1, $2, 'builder', 1, 1, 1, $3, NOW())",
            [
                builder.into(),
                builder_name(&site.name, stale.id).into(),
                site.resource_group_id.into(),
            ],
        ))
        .await
        .unwrap();

    harness.builds.reap_stale().await.unwrap();

    assert_eq!(deployment_of(&harness.db, stale.id).await.status, "failed");
    assert!(harness.scheduler.deleted.lock().unwrap().contains(&builder));
}
