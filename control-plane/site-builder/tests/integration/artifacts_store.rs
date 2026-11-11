use site_builder::artifacts::{exists, load, store};
use site_builder::error::ServiceError;
use uuid::Uuid;

use crate::common::{insert_deployment, insert_site, migrated_database, tar_gz};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn artifacts_are_stored_replaced_and_loaded() {
    let db = migrated_database().await;
    let site = insert_site(&db, "static", None).await;
    let deployment = insert_deployment(&db, site.id, "building", "production", "main").await;
    assert!(!exists(&db, deployment.id).await.unwrap());
    assert!(load(&db, deployment.id).await.unwrap().is_none());
    store(&db, deployment.id, tar_gz(&[("index.html", b"one")]))
        .await
        .unwrap();
    store(&db, deployment.id, tar_gz(&[("index.html", b"two!")]))
        .await
        .unwrap();
    assert!(exists(&db, deployment.id).await.unwrap());
    let archive = load(&db, deployment.id).await.unwrap().unwrap();
    assert_eq!(archive.files["index.html"], b"two!");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn invalid_archives_are_not_stored() {
    let db = migrated_database().await;
    let id = Uuid::new_v4();
    let error = store(&db, id, b"garbage".to_vec()).await.unwrap_err();
    assert!(matches!(error, ServiceError::Archive(_)));
    assert!(!exists(&db, id).await.unwrap());
}
