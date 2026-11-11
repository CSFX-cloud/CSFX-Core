use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use uuid::Uuid;

use crate::common;

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn issues_and_stores_certificate() {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;
    let pki = common::pki_service(db.clone());

    let issued = pki
        .issue_certificate(agent_id, &common::csr_pem())
        .await
        .unwrap();

    assert!(issued
        .certificate_pem
        .starts_with("-----BEGIN CERTIFICATE-----"));
    assert!(pki.ca_cert_pem().starts_with("-----BEGIN CERTIFICATE-----"));
    let stored = registry::db::certificates::get_active_certificate(&db, agent_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.serial_number, issued.serial_number);
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn rejects_invalid_csr() {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;

    let error = common::pki_service(db)
        .issue_certificate(agent_id, "not a csr")
        .await
        .err()
        .unwrap();

    assert!(error.to_string().starts_with("Invalid CSR PEM"));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn rotation_revokes_previous_certificate() {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;
    let pki = common::pki_service(db);
    let first = pki
        .issue_certificate(agent_id, &common::csr_pem())
        .await
        .unwrap();

    let second = pki
        .rotate_certificate(agent_id, &common::csr_pem())
        .await
        .unwrap();

    let revoked = pki.build_crl().await.unwrap();
    assert!(revoked.contains(&first.serial_number));
    assert!(!revoked.contains(&second.serial_number));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn revocation_requires_active_certificate() {
    let db = common::migrated_database().await;
    let agent_id = common::insert_agent(&db).await;
    let pki = common::pki_service(db);

    assert!(pki
        .revoke_agent_certificate(agent_id, "test".to_string())
        .await
        .is_err());
    pki.revoke_active_certificate(agent_id, "test".to_string())
        .await
        .unwrap();

    let issued = pki
        .issue_certificate(agent_id, &common::csr_pem())
        .await
        .unwrap();
    pki.revoke_active_certificate(agent_id, "test".to_string())
        .await
        .unwrap();
    assert!(pki
        .build_crl()
        .await
        .unwrap()
        .contains(&issued.serial_number));
}

#[tokio::test]
#[ignore = "requires postgres via DATABASE_URL"]
async fn proxy_ticket_payload_names_agent_and_workload() {
    let pki = common::pki_service(common::migrated_database().await);
    let agent_id = Uuid::new_v4();

    let ticket = pki.sign_proxy_ticket(agent_id, "workload-1").unwrap();

    let payload = String::from_utf8(URL_SAFE_NO_PAD.decode(ticket.payload).unwrap()).unwrap();
    assert!(payload.starts_with(&format!("{}.workload-1.", agent_id)));
    assert!(!ticket.signature.is_empty());
}
