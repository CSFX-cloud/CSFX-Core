use api_gateway::{
    export::{
        self,
        engine::Exporter,
        manifest::{Scope, ScopeKind},
        sink::Sink,
        sources,
        target::Target,
        RestoreOptions,
    },
    routes, service_client, AppState,
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use entity::entities::{buckets, volumes};
use sea_orm::{ActiveModelTrait, ActiveValue::Set};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

use crate::common::{harness, Harness};

const PASSPHRASE: &str = "correct horse battery";

type Import = (String, Vec<u8>);

#[derive(Clone, Default)]
struct Received {
    imports: Arc<Mutex<Vec<Import>>>,
}

fn payload(length: usize) -> Vec<u8> {
    (0..length).map(|n| (n % 251) as u8).collect()
}

async fn export_stream(Path(_id): Path<Uuid>) -> impl IntoResponse {
    payload(3 * 1024 * 1024 + 17)
}

async fn import_stream(
    State(received): State<Received>,
    Query(query): Query<HashMap<String, String>>,
    body: Body,
) -> impl IntoResponse {
    let name = query.get("name").cloned().unwrap_or_default();
    let data = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    received.imports.lock().unwrap().push((name, data.to_vec()));
    (StatusCode::CREATED, Json(json!({ "id": Uuid::new_v4() })))
}

async fn create_bucket(Json(request): Json<Value>) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(json!({ "id": Uuid::new_v4(), "name": request["name"] })),
    )
}

async fn import_bucket(
    State(received): State<Received>,
    Path(_id): Path<Uuid>,
    body: Body,
) -> impl IntoResponse {
    let data = axum::body::to_bytes(body, usize::MAX).await.unwrap();
    received
        .imports
        .lock()
        .unwrap()
        .push(("bucket".to_string(), data.to_vec()));
    Json(json!({ "objects": 2 }))
}

async fn serve(router: Router) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{address}")
}

async fn state_with_upstreams(harness: &Harness, volume_url: &str, bucket_url: &str) -> AppState {
    let previous: Vec<_> = ["VOLUME_MANAGER_URL", "OBJECT_STORAGE_URL"]
        .iter()
        .map(|name| (*name, std::env::var(name).ok()))
        .collect();
    std::env::set_var("VOLUME_MANAGER_URL", volume_url);
    std::env::set_var("OBJECT_STORAGE_URL", bucket_url);
    let service_client = service_client::ServiceClient::new();
    for (name, value) in previous {
        match value {
            Some(value) => std::env::set_var(name, value),
            None => std::env::remove_var(name),
        }
    }
    AppState {
        db_conn: harness.db.clone(),
        agent_db: harness.db.clone(),
        service_client,
        default_org_id: harness.org_id,
        agent_stream_registry: routes::agent_stream::AgentStreamRegistry::new(),
        openbao: None,
        site_builder: None,
    }
}

async fn insert_volume(harness: &Harness) -> Uuid {
    let id = Uuid::new_v4();
    volumes::ActiveModel {
        id: Set(id),
        name: Set("data disk".to_string()),
        size_gb: Set(7),
        pool: Set("rbd".to_string()),
        image_name: Set(format!("img-{id}")),
        status: Set("available".to_string()),
        created_at: Set(Utc::now().naive_utc()),
        ..Default::default()
    }
    .insert(&harness.db)
    .await
    .unwrap();
    id
}

async fn insert_bucket(harness: &Harness) -> Uuid {
    let id = Uuid::new_v4();
    buckets::ActiveModel {
        id: Set(id),
        name: Set("media".to_string()),
        garage_bucket_id: Set(None),
        global_alias: Set(format!("media-{id}")),
        exposure: Set("private".to_string()),
        quota_max_size: Set(None),
        quota_max_objects: Set(None),
        status: Set("active".to_string()),
        master_key_id: Set(Some("GKsecretid".to_string())),
        master_key_secret_encrypted: Set(Some(vec![1, 2, 3])),
        organization_id: Set(None),
        resource_group_id: Set(None),
        created_at: Set(Utc::now().naive_utc()),
        updated_at: Set(None),
    }
    .insert(&harness.db)
    .await
    .unwrap();
    id
}

fn data_options() -> RestoreOptions {
    RestoreOptions {
        target: Some("data".to_string()),
        ..RestoreOptions::default()
    }
}

async fn configure_target(harness: &Harness, root: &std::path::Path) {
    let body = json!({
        "kind": "local",
        "path": root,
        "passphrase": PASSPHRASE,
        "default": true
    });
    let (status, _) = harness
        .admin(
            axum::http::Method::PUT,
            "/api/admin/export-targets/data",
            Some(body),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn volume_and_bucket_exports_round_trip_through_upstreams() {
    let harness = harness().await;
    let received = Received::default();
    let volume_url = serve(
        Router::new()
            .route("/volumes/{id}/export", get(export_stream))
            .route("/volumes/import", post(import_stream))
            .with_state(received.clone()),
    )
    .await;
    let bucket_url = serve(
        Router::new()
            .route("/buckets/{id}/export", get(export_stream))
            .route("/buckets", post(create_bucket))
            .route("/buckets/{id}/import", post(import_bucket))
            .with_state(received.clone()),
    )
    .await;
    let state = state_with_upstreams(&harness, &volume_url, &bucket_url).await;
    let root = std::env::temp_dir().join(format!("csfx-data-{}", Uuid::new_v4()));
    configure_target(&harness, &root).await;

    let volume_id = insert_volume(&harness).await;
    let scope = Scope {
        kind: ScopeKind::Volume,
        id: Some(volume_id),
    };
    let export_id = Uuid::new_v4();
    let manifest = export::run_export(&state, export_id, scope, Some("data"))
        .await
        .unwrap();
    assert_eq!(manifest.artifacts.len(), 2);
    assert!(manifest.encryption.is_some());
    let image = std::fs::read(
        root.join(export_id.to_string())
            .join(format!("volumes/{volume_id}.img")),
    )
    .unwrap();
    assert_ne!(
        image,
        payload(3 * 1024 * 1024 + 17),
        "image must be stored encrypted"
    );

    let options = data_options();
    let outcome = export::restore(&state, export_id, options).await.unwrap();
    assert!(outcome["volume_id"].is_string());
    {
        let imports = received.imports.lock().unwrap();
        let (name, data) = imports.last().unwrap();
        assert_eq!(name, "data disk-restored");
        assert_eq!(data, &payload(3 * 1024 * 1024 + 17));
    }

    let bucket_id = insert_bucket(&harness).await;
    let scope = Scope {
        kind: ScopeKind::Bucket,
        id: Some(bucket_id),
    };
    let bucket_export = Uuid::new_v4();
    export::run_export(&state, bucket_export, scope, Some("data"))
        .await
        .unwrap();
    let row = std::fs::read(
        root.join(bucket_export.to_string())
            .join(format!("buckets/{bucket_id}.json")),
    )
    .unwrap();
    let row = String::from_utf8_lossy(&row);
    assert!(!row.contains("GKsecretid"));
    let outcome = export::restore(&state, bucket_export, data_options())
        .await
        .unwrap();
    assert_eq!(outcome["objects"], 2);
    assert_eq!(
        received.imports.lock().unwrap().last().unwrap().1,
        payload(3 * 1024 * 1024 + 17)
    );

    let tampered = root
        .join(export_id.to_string())
        .join(format!("volumes/{volume_id}.img"));
    let mut bytes = std::fs::read(&tampered).unwrap();
    let middle = bytes.len() / 2;
    bytes[middle] ^= 1;
    std::fs::write(&tampered, bytes).unwrap();
    let failure = export::restore(&state, export_id, data_options()).await;
    assert!(failure.is_err());
    std::fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn resource_group_scope_requires_an_existing_group() {
    let harness = harness().await;
    let missing = json!({ "scope": { "kind": "resource_group", "id": Uuid::new_v4() } });
    let (status, _) = harness
        .admin(axum::http::Method::POST, "/api/exports", Some(missing))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

fn install_fake_pg_dump(root: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(root).unwrap();
    let script = root.join("pg_dump");
    std::fs::write(&script, "#!/bin/sh\nprintf PGDMP\nhead -c 5000 /dev/zero\n").unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("CSFX_PG_DUMP", &script);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL and etcd via ETCD_ENDPOINTS"]
async fn database_and_etcd_artifacts_are_captured() {
    let _harness = harness().await;
    let root = std::env::temp_dir().join(format!("csfx-full-{}", Uuid::new_v4()));
    install_fake_pg_dump(&root);
    let export_id = Uuid::new_v4();
    let target = Target {
        sink: Sink::Local(root.clone()),
        passphrase: Some(PASSPHRASE.to_string()),
    };
    let mut exporter = Exporter::new(export_id, target).unwrap();
    sources::database::export(&mut exporter).await.unwrap();
    sources::etcd::export(&mut exporter).await.unwrap();
    let scope = Scope {
        kind: ScopeKind::Full,
        id: None,
    };
    let manifest = exporter.finish("test".to_string(), scope).await.unwrap();
    let kinds: Vec<_> = manifest.artifacts.iter().map(|a| a.kind.as_str()).collect();
    assert_eq!(kinds, ["database", "etcd"]);
    assert!(manifest.artifacts[0].bytes > 1000);
    std::fs::remove_dir_all(root).unwrap();
}
