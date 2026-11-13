use crate::common;

use registry::services::{
    api_keys::ApiKeyManager,
    bootstrap_tokens::{BootstrapTokenManager, ROLE_MASTER, ROLE_WORKER},
    tokens::TokenManager,
};
use sea_orm::EntityTrait;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registration_token_is_consumed_once() {
    let manager = TokenManager::new(common::migrated_database().await);
    let agent_id = Uuid::new_v4();
    let token = manager
        .create_token(
            agent_id,
            format!("name-{}", agent_id.simple()),
            "host".to_string(),
            None,
            "tester".to_string(),
            1,
        )
        .await;

    let consumed = manager
        .validate_and_consume_token(&token.token)
        .await
        .unwrap();
    assert_eq!(consumed.agent_id, agent_id);
    let error = manager
        .validate_and_consume_token(&token.token)
        .await
        .unwrap_err();
    assert_eq!(error, "Token is already used");
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registration_token_rejects_unknown_and_expired() {
    let manager = TokenManager::new(common::migrated_database().await);
    let error = manager
        .validate_and_consume_token("reg_unknown")
        .await
        .unwrap_err();
    assert_eq!(error, "Token not found");

    let expired = manager
        .create_token(
            Uuid::new_v4(),
            "expired".to_string(),
            "host".to_string(),
            None,
            "tester".to_string(),
            -1,
        )
        .await;
    let error = manager
        .validate_and_consume_token(&expired.token)
        .await
        .unwrap_err();
    assert_eq!(error, "Token is expired");
    assert!(manager.cleanup_expired().await >= 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn registration_token_is_listed_until_used() {
    let manager = TokenManager::new(common::migrated_database().await);
    let token = manager
        .create_token(
            Uuid::new_v4(),
            "listed".to_string(),
            "host".to_string(),
            Some("note".to_string()),
            "tester".to_string(),
            1,
        )
        .await;

    let listed = manager.list_tokens().await;
    assert!(listed
        .iter()
        .any(|candidate| candidate.token == token.token));
    manager
        .validate_and_consume_token(&token.token)
        .await
        .unwrap();
    let listed = manager.list_tokens().await;
    assert!(!listed
        .iter()
        .any(|candidate| candidate.token == token.token));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_token_enforces_use_limit() {
    let manager = BootstrapTokenManager::new(common::migrated_database().await);
    let token = manager
        .create(None, "tester".to_string(), 1, 2, ROLE_MASTER.to_string())
        .await
        .unwrap();

    assert!(BootstrapTokenManager::is_bootstrap_token(&token.token));
    assert_eq!(
        manager.validate_and_use(&token.token).await.unwrap(),
        ROLE_MASTER
    );
    assert_eq!(
        manager.validate_and_use(&token.token).await.unwrap(),
        ROLE_MASTER
    );
    assert_eq!(
        manager.validate_and_use(&token.token).await,
        Err("Bootstrap token use limit reached".to_string())
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_token_rejects_revoked_expired_and_unknown() {
    let manager = BootstrapTokenManager::new(common::migrated_database().await);
    let revoked = manager
        .create(None, "tester".to_string(), 1, 5, ROLE_WORKER.to_string())
        .await
        .unwrap();
    manager.revoke(revoked.id).await.unwrap();
    assert_eq!(
        manager.validate_and_use(&revoked.token).await,
        Err("Bootstrap token has been revoked".to_string())
    );

    let expired = manager
        .create(None, "tester".to_string(), -1, 5, ROLE_WORKER.to_string())
        .await
        .unwrap();
    assert_eq!(
        manager.validate_and_use(&expired.token).await,
        Err("Bootstrap token has expired".to_string())
    );
    assert_eq!(
        manager.validate_and_use("csfx-bootstrap.unknown").await,
        Err("Bootstrap token not found".to_string())
    );
    assert!(manager.cleanup_expired().await >= 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_token_listing_skips_revoked() {
    let manager = BootstrapTokenManager::new(common::migrated_database().await);
    let kept = manager
        .create(None, "tester".to_string(), 1, 5, ROLE_WORKER.to_string())
        .await
        .unwrap();
    let revoked = manager
        .create(None, "tester".to_string(), 1, 5, ROLE_WORKER.to_string())
        .await
        .unwrap();
    manager.revoke(revoked.id).await.unwrap();

    let listed = manager.list().await;
    assert!(listed.iter().any(|candidate| candidate.id == kept.id));
    assert!(!listed.iter().any(|candidate| candidate.id == revoked.id));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn api_key_validates_until_revoked() {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;
    let manager = ApiKeyManager::new(db);
    let key = manager.create_key(agent_id).await;

    assert_eq!(manager.validate_key(&key.key).await, Ok(agent_id));
    manager.revoke_all_keys(agent_id).await.unwrap();
    assert_eq!(
        manager.validate_key(&key.key).await,
        Err(registry::services::api_keys::ApiKeyError::Invalid)
    );
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn parallel_registrations_cannot_exceed_the_use_limit() {
    let manager = std::sync::Arc::new(BootstrapTokenManager::new(
        common::migrated_database().await,
    ));
    let token = manager
        .create(None, "tester".to_string(), 1, 1, ROLE_WORKER.to_string())
        .await
        .unwrap();

    let attempts: Vec<_> = (0..20)
        .map(|_| {
            let manager = manager.clone();
            let secret = token.token.clone();
            tokio::spawn(async move { manager.validate_and_use(&secret).await })
        })
        .collect();
    let mut successes = 0;
    for attempt in attempts {
        if attempt.await.unwrap().is_ok() {
            successes += 1;
        }
    }

    assert_eq!(successes, 1);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn bootstrap_tokens_are_stored_hashed_and_never_listed() {
    let db = common::migrated_database().await;
    let manager = BootstrapTokenManager::new(db.clone());
    let token = manager
        .create(None, "tester".to_string(), 1, 1, ROLE_WORKER.to_string())
        .await
        .unwrap();

    let stored = entity::bootstrap_tokens::Entity::find_by_id(token.id)
        .one(&db)
        .await
        .unwrap()
        .unwrap();
    assert_ne!(stored.token_hash, token.token);
    assert_eq!(stored.token_hash.len(), 64);
    let listed = manager.list().await;
    let entry = listed.iter().find(|entry| entry.id == token.id).unwrap();
    assert!(entry.token.is_empty());
}
