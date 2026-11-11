use axum::http::{Method, StatusCode};
use sea_orm::EntityTrait;
use serde_json::json;
use uuid::Uuid;

use crate::common::harness;
use crate::support_b::{agent_with_key, full_harness, raw, AgentSpec};

fn spec(role: &'static str, tunnel_ip: Option<&'static str>, age_minutes: i64) -> AgentSpec {
    AgentSpec {
        role,
        tunnel_ip,
        age_minutes,
        cordoned: false,
    }
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn join_tokens_validate_input_before_calling_the_registry() {
    let harness = full_harness().await;
    let uri = "/api/cluster/join-tokens";
    let bearer = format!("Bearer {}", harness.token);
    let host = [
        ("host", "10.20.30.40:8000"),
        ("authorization", bearer.as_str()),
    ];

    for body in [
        json!({"ttl_hours": 0}),
        json!({"ttl_hours": 169}),
        json!({"role": "owner"}),
    ] {
        let response = raw(&harness, Method::POST, uri, &host, Some(body)).await;
        assert_eq!(response.status, StatusCode::BAD_REQUEST);
    }
    let missing_host = harness.post(uri, json!({})).await;
    assert_eq!(missing_host.0, StatusCode::BAD_REQUEST);
    let loopback = [
        ("host", "localhost:8000"),
        ("authorization", bearer.as_str()),
    ];
    let response = raw(&harness, Method::POST, uri, &loopback, Some(json!({}))).await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn join_tokens_surface_an_incomplete_registry_answer() {
    let harness = full_harness().await;
    let bearer = format!("Bearer {}", harness.token);
    let authorized = [
        ("host", "10.20.30.40:8000"),
        ("authorization", bearer.as_str()),
    ];
    let response = raw(
        &harness,
        Method::POST,
        "/api/cluster/join-tokens",
        &authorized,
        Some(json!({"role": "master"})),
    )
    .await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn nodes_can_only_be_removed_when_cordoned_and_idle() {
    let harness = full_harness().await;
    let unknown = format!("/api/cluster/nodes/{}", Uuid::new_v4());
    assert_eq!(
        harness.admin(Method::DELETE, &unknown, None).await.0,
        StatusCode::NOT_FOUND
    );

    let (active, _) = agent_with_key(&harness, spec("worker", Some("10.250.1.1"), 0)).await;
    let uri = format!("/api/cluster/nodes/{active}");
    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::CONFLICT
    );

    for role in ["worker", "master"] {
        let cordoned = AgentSpec {
            cordoned: true,
            ..spec(role, Some("10.250.9.9"), 0)
        };
        let (id, _) = agent_with_key(&harness, cordoned).await;
        let uri = format!("/api/cluster/nodes/{id}");
        assert_eq!(
            harness.admin(Method::DELETE, &uri, None).await.0,
            StatusCode::NO_CONTENT,
            "{role}"
        );
    }
    let bare = AgentSpec {
        cordoned: true,
        ..spec("master", None, 0)
    };
    let (id, _) = agent_with_key(&harness, bare).await;
    let uri = format!("/api/cluster/nodes/{id}");
    assert_eq!(
        harness.admin(Method::DELETE, &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    let kept = entity::entities::agents::Entity::find_by_id(id)
        .one(&harness.db)
        .await
        .unwrap();
    assert!(kept.is_some());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn masters_listing_reads_etcd_members() {
    let harness = harness().await;
    let (status, body) = harness.get("/api/cluster/masters").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["voters"].is_number());
    assert!(body["members"].is_array());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn master_join_and_promotion_check_the_calling_agent() {
    let harness = full_harness().await;
    let call = |key: String, uri: &'static str| {
        let harness = &harness;
        async move {
            raw(
                harness,
                Method::POST,
                uri,
                &[("x-api-key", key.as_str())],
                None,
            )
            .await
            .status
        }
    };

    let (_, worker) = agent_with_key(&harness, spec("worker", Some("10.250.2.1"), 0)).await;
    let (_, stale) = agent_with_key(&harness, spec("master", Some("10.250.2.2"), 120)).await;
    let (_, bare) = agent_with_key(&harness, spec("master", None, 0)).await;
    let (_, fresh) = agent_with_key(&harness, spec("master", Some("10.250.2.3"), 0)).await;

    for uri in ["/api/cluster/masters/join", "/api/cluster/masters/promote"] {
        assert_eq!(
            call("unknown".to_string(), uri).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(call(worker.clone(), uri).await, StatusCode::FORBIDDEN);
    }
    assert_eq!(
        call(stale.clone(), "/api/cluster/masters/join").await,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        call(bare.clone(), "/api/cluster/masters/join").await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(bare, "/api/cluster/masters/promote").await,
        StatusCode::CONFLICT
    );
    assert_eq!(
        call(fresh.clone(), "/api/cluster/masters/join").await,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        call(fresh, "/api/cluster/masters/promote").await,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        raw(
            &harness,
            Method::POST,
            "/api/cluster/masters/join",
            &[],
            None
        )
        .await
        .status,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn cluster_routes_require_system_permission() {
    let harness = harness().await;
    let limited = harness.limited_token().await;
    let id = Uuid::new_v4();
    let cases = [
        (Method::POST, "/api/cluster/join-tokens".to_string()),
        (Method::DELETE, format!("/api/cluster/nodes/{id}")),
        (Method::GET, "/api/cluster/masters".to_string()),
    ];
    for (method, uri) in cases {
        let body = Some(json!({}));
        assert_eq!(
            harness
                .send(method.clone(), &uri, None, body.clone())
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{uri}"
        );
        assert_eq!(
            harness.send(method, &uri, Some(&limited), body).await.0,
            StatusCode::FORBIDDEN,
            "{uri}"
        );
    }
}
