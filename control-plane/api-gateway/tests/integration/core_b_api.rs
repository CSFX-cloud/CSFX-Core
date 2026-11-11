use api_gateway::auth::crypto::{
    decrypt_password, generate_salt, hash_password, verify_password, RsaKeyPair,
};
use api_gateway::auth::jwt::{
    create_exec_ticket, create_node_metrics_ticket, create_vnc_ticket, verify_exec_ticket,
    verify_node_metrics_ticket, verify_vnc_ticket,
};
use api_gateway::auth::rate_limit::{JwtOrIpKeyExtractor, RateLimitKey};
use api_gateway::rbac_service::RbacService;
use api_gateway::{cluster_bundle, init, recovery, tls, vault_service, vault_store};
use axum::http::{Method, Request, StatusCode};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use serde_json::json;
use tower_governor::key_extractor::KeyExtractor;
use uuid::Uuid;

use crate::common::{create_role, create_user, encrypt_password, harness};
use crate::support_b::{full_harness, full_state, resource_group};

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn rbac_service_checks_and_manages_roles() {
    let harness = harness().await;
    let service = RbacService::new(harness.db.clone());

    assert!(service
        .has_permission(harness.admin_id, harness.org_id, "vaults", "*")
        .await
        .is_ok());
    let role_id = create_role(&harness.db, harness.org_id).await;
    let (user_id, _) = create_user(&harness.db, harness.org_id, role_id).await;
    let allowed = service
        .has_permission(user_id, harness.org_id, "secrets", "read")
        .await
        .unwrap();
    assert!(!allowed);
    let stranger = service
        .has_permission(Uuid::new_v4(), harness.org_id, "secrets", "read")
        .await
        .unwrap();
    assert!(!stranger);

    let permission = entity::Permission::find()
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    service
        .assign_permissions_to_role(role_id, vec![permission.id])
        .await
        .unwrap();
    let granted = service.get_role_permissions(role_id).await.unwrap();
    assert_eq!(granted.len(), 1);
    let now_allowed = service
        .has_permission(
            user_id,
            harness.org_id,
            &permission.resource,
            &permission.action,
        )
        .await
        .unwrap();
    assert!(now_allowed);

    let role = service
        .get_user_role(user_id, harness.org_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(role.id, role_id);
    assert!(service
        .get_user_role(Uuid::new_v4(), harness.org_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        service.get_user_organizations(user_id).await.unwrap().len(),
        1
    );
    let members = service
        .get_organization_members(harness.org_id)
        .await
        .unwrap();
    assert!(members
        .iter()
        .any(|(membership, _)| membership.user_id == user_id));

    let created = service
        .create_role(
            harness.org_id,
            format!("r-{}", Uuid::new_v4().simple()),
            None,
            false,
        )
        .await
        .unwrap();
    service
        .assign_role_to_user(user_id, harness.org_id, created.id)
        .await
        .unwrap();
    assert_eq!(
        service
            .get_user_role(user_id, harness.org_id)
            .await
            .unwrap()
            .unwrap()
            .id,
        created.id
    );
    service
        .remove_user_from_organization(user_id, harness.org_id)
        .await
        .unwrap();
    assert!(service
        .get_user_role(user_id, harness.org_id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn database_initialisation_is_idempotent() {
    let harness = harness().await;
    let first = init::initialize_database(&harness.db).await.unwrap();
    let second = init::initialize_database(&harness.db).await.unwrap();
    assert_eq!(first, second);
    assert_eq!(first, harness.org_id);
    let admin = entity::Role::find()
        .filter(entity::role::Column::Name.eq("Admin"))
        .filter(entity::role::Column::OrganizationId.eq(first))
        .all(&harness.db)
        .await
        .unwrap();
    assert_eq!(admin.len(), 1);
}

#[test]
fn tickets_are_bound_to_their_purpose() {
    std::env::set_var("JWT_SECRET", "integration-test-secret");
    let (workload, user) = (Uuid::new_v4(), Uuid::new_v4());
    let exec = create_exec_ticket(workload, user).unwrap();
    let vnc = create_vnc_ticket(workload, user).unwrap();
    let metrics = create_node_metrics_ticket(workload, user).unwrap();

    assert_eq!(verify_exec_ticket(&exec, workload).unwrap().user_id, user);
    assert_eq!(
        verify_vnc_ticket(&vnc, workload).unwrap().workload_id,
        workload
    );
    assert_eq!(
        verify_node_metrics_ticket(&metrics, workload)
            .unwrap()
            .agent_id,
        workload
    );
    assert!(verify_exec_ticket(&exec, Uuid::new_v4()).is_err());
    assert!(verify_exec_ticket(&vnc, workload).is_err());
    assert!(verify_vnc_ticket(&metrics, workload).is_err());
    assert!(verify_node_metrics_ticket("garbage", workload).is_err());
}

#[test]
fn passwords_and_rsa_round_trip() {
    let salt = generate_salt();
    let hashed = hash_password("hunter2-hunter2", &salt).unwrap();
    assert!(verify_password("hunter2-hunter2", &salt, &hashed).unwrap());
    assert!(!verify_password("other", &salt, &hashed).unwrap());

    let pair = RsaKeyPair::generate().unwrap();
    let encrypted = encrypt_password(&pair.public_key, "secret-value");
    assert_eq!(
        decrypt_password(&encrypted, &pair.private_key).unwrap(),
        "secret-value"
    );
    assert_eq!(
        RsaKeyPair::from_private_key(&pair.private_key)
            .unwrap()
            .public_key,
        pair.public_key
    );
    assert!(decrypt_password("not-base64!", &pair.private_key).is_err());
    assert!(RsaKeyPair::from_private_key("garbage").is_err());
}

#[test]
fn rate_limit_keys_prefer_user_then_agent_then_address() {
    std::env::set_var("JWT_SECRET", "integration-test-secret");
    let extractor = JwtOrIpKeyExtractor::new();
    let user = Uuid::new_v4();
    let token = api_gateway::auth::jwt::create_jwt(user, "name".to_string()).unwrap();

    let by_user = Request::builder()
        .header("authorization", format!("Bearer {token}"))
        .body(())
        .unwrap();
    assert_eq!(
        extractor.extract(&by_user).unwrap(),
        RateLimitKey::User(user)
    );
    let by_key = Request::builder()
        .header("x-api-key", "abc")
        .body(())
        .unwrap();
    assert!(matches!(
        extractor.extract(&by_key).unwrap(),
        RateLimitKey::AgentApiKey(_)
    ));
    let mut anonymous = Request::builder()
        .header("authorization", "Bearer broken")
        .body(())
        .unwrap();
    anonymous
        .extensions_mut()
        .insert(axum::extract::ConnectInfo(
            "10.1.2.3:99".parse::<std::net::SocketAddr>().unwrap(),
        ));
    assert!(matches!(
        extractor.extract(&anonymous).unwrap(),
        RateLimitKey::Ip(_)
    ));
    assert!(extractor
        .extract(&Request::builder().body(()).unwrap())
        .is_err());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn tls_configuration_is_signed_by_the_cluster_ca() {
    let harness = full_harness().await;
    assert_eq!(harness.get("/api/system/health").await.0, StatusCode::OK);
    let (cert, key) = tls::load_ca_pem().await.unwrap();
    assert!(cert.contains("BEGIN CERTIFICATE") && key.contains("PRIVATE KEY"));
    tls::generate_tls_config().await.unwrap();
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn recovery_commands_dispatch_and_reset_admins() {
    let harness = harness().await;
    let strings = |values: &[&str]| {
        values
            .iter()
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(recovery::dispatch(&[]).await, None);
    assert_eq!(recovery::dispatch(&strings(&["--unknown"])).await, None);
    assert_eq!(
        recovery::dispatch(&strings(&["--reset-admin"])).await,
        Some(0)
    );
    assert_eq!(
        recovery::dispatch(&strings(&["--reset-admin", "nobody@csfx.test"])).await,
        Some(4)
    );

    let (_, name, _) = harness.user_with_token().await;
    assert_eq!(
        recovery::dispatch(&strings(&["--reset-admin", &name])).await,
        Some(0)
    );
    let reset = entity::User::find()
        .filter(entity::user::Column::Name.eq(name))
        .one(&harness.db)
        .await
        .unwrap()
        .unwrap();
    assert!(reset.force_password_change);
    assert!(!reset.two_factor_enabled);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn cluster_secrets_require_a_complete_environment() {
    let harness = full_harness().await;
    assert_eq!(harness.get("/api/system/health").await.0, StatusCode::OK);
    let loaded = cluster_bundle::ClusterSecrets::load();
    assert!(loaded.is_err());
    assert!(cluster_bundle::ClusterVip::from_values(Some("10.0.0.5".into()), None, None).is_some());
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn expired_soft_deleted_secrets_are_purged() {
    let harness = full_harness().await;
    let state = full_state(&harness);
    let group = resource_group(&harness).await;
    let base = format!("/api/resource-groups/{group}/vaults");
    let (_, vault) = harness.post(&base, json!({"name": "purge"})).await;
    let vault_id: Uuid = vault["id"].as_str().unwrap().parse().unwrap();
    let uri = format!("{base}/{vault_id}/secrets");
    for name in ["OLD", "KEEP"] {
        harness
            .post(&uri, json!({"name": name, "value": "v"}))
            .await;
    }
    assert_eq!(
        harness
            .admin(Method::DELETE, &format!("{uri}/OLD"), None)
            .await
            .0,
        StatusCode::NO_CONTENT
    );

    let scope = vault_store::VaultScope {
        resource_group_id: group,
        vault_id,
    };
    let bao = state.openbao.as_ref().unwrap();
    assert_eq!(vault_store::purge_expired(bao, scope, 30).await.unwrap(), 1);
    assert_eq!(
        vault_store::list_names(bao, scope).await.unwrap(),
        vec!["KEEP".to_string()]
    );
    assert!(bao.is_available().await);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn pending_resource_group_deletions_are_completed_by_the_job() {
    let harness = full_harness().await;
    let state = full_state(&harness);
    let group = resource_group(&harness).await;
    let base = format!("/api/resource-groups/{group}/vaults");
    let (_, vault) = harness.post(&base, json!({"name": "doomed"})).await;
    harness
        .post(
            &format!("{base}/{}/secrets", vault["id"].as_str().unwrap()),
            json!({"name": "S", "value": "v"}),
        )
        .await;
    vault_service::teardown_resource_group(&state, group)
        .await
        .unwrap();

    let mut active: entity::resource_groups::ActiveModel =
        entity::ResourceGroups::find_by_id(group)
            .one(&harness.db)
            .await
            .unwrap()
            .unwrap()
            .into();
    active.status = sea_orm::ActiveValue::Set("deleting".to_string());
    sea_orm::ActiveModelTrait::update(active, &harness.db)
        .await
        .unwrap();

    vault_service::spawn_deletion_job(state);
    for _ in 0..50 {
        let remaining = entity::ResourceGroups::find_by_id(group)
            .one(&harness.db)
            .await
            .unwrap();
        if remaining.is_none() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!("resource group was not removed by the deletion job");
}
