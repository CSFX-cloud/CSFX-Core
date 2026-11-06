use std::sync::Arc;

use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use site_builder::artifacts;
use site_builder::edge::Edge;
use site_builder::server::edge_router;
use tower::ServiceExt;
use uuid::Uuid;

use crate::common::{
    insert_deployment, insert_domain, insert_site, migrated_database, point_site_at, tar_gz,
};

struct Published {
    host: String,
    deployment: Uuid,
}

async fn publish(db: &DatabaseConnection, files: &[(&str, &[u8])]) -> Published {
    let site = insert_site(db, "static", None).await;
    let deployment = insert_deployment(db, site.id, "ready", "production", "main").await;
    artifacts::store(db, deployment.id, tar_gz(files))
        .await
        .unwrap();
    let host = insert_domain(db, site.id, "production", None).await;
    point_site_at(db, site, deployment.id).await;
    Published {
        host,
        deployment: deployment.id,
    }
}

async fn get(app: &Router, host: Option<&str>, path: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut builder = Request::builder().uri(path);
    if let Some(host) = host {
        builder = builder.header("host", host);
    }
    let response = app
        .clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let (parts, body) = response.into_parts();
    (
        parts.status,
        parts.headers,
        body.collect().await.unwrap().to_bytes().to_vec(),
    )
}

async fn edge(db: &DatabaseConnection) -> (Router, Arc<Edge>) {
    let edge = Arc::new(Edge::new(db.clone()));
    (edge_router(edge.clone()), edge)
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pages_and_assets_are_served_with_cache_headers() {
    let db = migrated_database().await;
    let published = publish(
        &db,
        &[("index.html", b"<h1>home</h1>"), ("app.js", b"let a=1;")],
    )
    .await;
    let (app, _) = edge(&db).await;
    let host = format!("{}:8008", published.host.to_uppercase());
    let (status, headers, body) = get(&app, Some(&host), "/").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"<h1>home</h1>");
    assert_eq!(headers["content-type"], "text/html");
    assert_eq!(
        headers["cache-control"],
        "public, max-age=0, must-revalidate"
    );
    assert_eq!(
        headers["x-csfx-deployment"],
        published.deployment.to_string()
    );
    let (_, headers, _) = get(&app, Some(&published.host), "/app.js").await;
    assert_eq!(headers["cache-control"], "public, max-age=3600");
    assert_eq!(headers["x-content-type-options"], "nosniff");
    let (status, _, _) = get(&app, Some(&published.host), "/missing.png").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _, body) = get(&app, Some(&published.host), "/client/route").await;
    assert_eq!((status, body), (StatusCode::OK, b"<h1>home</h1>".to_vec()));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn unknown_hosts_and_missing_headers_are_rejected() {
    let db = migrated_database().await;
    let (app, _) = edge(&db).await;
    let (status, _, _) = get(&app, None, "/").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _, _) = get(&app, Some("nobody.invalid"), "/").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn container_sites_and_artifactless_deployments_are_not_served() {
    let db = migrated_database().await;
    let (app, _) = edge(&db).await;
    let container = insert_site(&db, "container", Some(3000)).await;
    let deployment = insert_deployment(&db, container.id, "ready", "production", "main").await;
    let host = insert_domain(&db, container.id, "production", None).await;
    point_site_at(&db, container, deployment.id).await;
    assert_eq!(get(&app, Some(&host), "/").await.0, StatusCode::NOT_FOUND);
    let empty = insert_site(&db, "static", None).await;
    let bare = insert_deployment(&db, empty.id, "ready", "production", "main").await;
    let bare_host = insert_domain(&db, empty.id, "production", None).await;
    point_site_at(&db, empty, bare.id).await;
    assert_eq!(
        get(&app, Some(&bare_host), "/").await.0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn branch_domains_serve_the_latest_ready_deployment_of_the_branch() {
    let db = migrated_database().await;
    let site = insert_site(&db, "static", None).await;
    let old = insert_deployment(&db, site.id, "ready", "preview", "feature").await;
    artifacts::store(&db, old.id, tar_gz(&[("index.html", b"old")]))
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    let latest = insert_deployment(&db, site.id, "ready", "preview", "feature").await;
    artifacts::store(&db, latest.id, tar_gz(&[("index.html", b"new")]))
        .await
        .unwrap();
    let host = insert_domain(&db, site.id, "branch", Some("feature")).await;
    let (app, _) = edge(&db).await;
    let (status, _, body) = get(&app, Some(&host), "/").await;
    assert_eq!((status, body), (StatusCode::OK, b"new".to_vec()));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn served_bytes_are_flushed_to_usage_counters() {
    let db = migrated_database().await;
    let published = publish(&db, &[("index.html", b"12345")]).await;
    let (app, edge) = edge(&db).await;
    get(&app, Some(&published.host), "/").await;
    get(&app, Some(&published.host), "/").await;
    edge.flush_usage().await;
    let statement = Statement::from_sql_and_values(
        db.get_database_backend(),
        "SELECT COALESCE(SUM(bytes_out), 0)::bigint FROM site_usage_hourly u JOIN site_deployments d ON d.site_id = u.site_id WHERE d.id = $1",
        [published.deployment.into()],
    );
    let row = db.query_one(statement).await.unwrap().unwrap();
    assert_eq!(row.try_get_by_index::<i64>(0).unwrap(), 10);
}
