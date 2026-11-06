use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde_json::json;
use site_builder::error::ServiceError;
use site_builder::scheduler::SchedulerClient;
use uuid::Uuid;

use crate::common::serve;

async fn client_for(router: Router) -> SchedulerClient {
    let address = serve(router).await;
    SchedulerClient::new(&format!("http://{address}/")).unwrap()
}

#[tokio::test]
async fn create_returns_the_workload_id() {
    let id = Uuid::new_v4();
    let router = Router::new().route(
        "/workloads",
        post(move || async move { Json(json!({ "workload_id": id })) }),
    );
    let client = client_for(router).await;
    assert_eq!(client.create(&json!({})).await.unwrap(), id);
}

#[tokio::test]
async fn create_surfaces_rejections_and_malformed_answers() {
    let rejected = Router::new().route(
        "/workloads",
        post(|| async {
            (
                StatusCode::BAD_REQUEST,
                Json(json!({ "error": "bad spec" })),
            )
        }),
    );
    let error = client_for(rejected)
        .await
        .create(&json!({}))
        .await
        .unwrap_err();
    assert!(matches!(&error, ServiceError::Scheduler(message) if message == "bad spec"));
    let anonymous = Router::new().route(
        "/workloads",
        post(|| async { (StatusCode::BAD_REQUEST, Json(json!({}))) }),
    );
    let error = client_for(anonymous)
        .await
        .create(&json!({}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("request rejected"));
    let empty = Router::new().route("/workloads", post(|| async { Json(json!({})) }));
    let error = client_for(empty)
        .await
        .create(&json!({}))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("no workload id"));
}

#[tokio::test]
async fn delete_treats_missing_workloads_as_deleted() {
    let missing = Router::new().route(
        "/workloads/{id}",
        delete(|| async { StatusCode::NOT_FOUND }),
    );
    assert!(client_for(missing)
        .await
        .delete(Uuid::new_v4())
        .await
        .is_ok());
    let broken = Router::new().route(
        "/workloads/{id}",
        delete(|| async { StatusCode::INTERNAL_SERVER_ERROR }),
    );
    let error = client_for(broken)
        .await
        .delete(Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("500"));
}

#[tokio::test]
async fn status_finds_the_matching_workload() {
    let id = Uuid::new_v4();
    let listing =
        json!([{ "id": Uuid::new_v4(), "status": "pending" }, { "id": id, "status": "running" }]);
    let router = Router::new().route("/workloads", get(move || async move { Json(listing) }));
    let client = client_for(router).await;
    assert_eq!(client.status(id).await.unwrap().as_deref(), Some("running"));
    assert_eq!(client.status(Uuid::new_v4()).await.unwrap(), None);
}

#[tokio::test]
async fn unreachable_schedulers_are_reported() {
    let client = SchedulerClient::new("http://127.0.0.1:1").unwrap();
    assert!(client.create(&json!({})).await.is_err());
    assert!(client.delete(Uuid::new_v4()).await.is_err());
    assert!(client.status(Uuid::new_v4()).await.is_err());
}
