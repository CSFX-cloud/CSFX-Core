use anyhow::{anyhow, Result};
use rcgen::{CertificateSigningRequestParams, PublicKeyData};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

fn same_public_key(first_csr_pem: &str, second_csr_pem: &str) -> Result<bool> {
    let first = CertificateSigningRequestParams::from_pem(first_csr_pem)
        .map_err(|e| anyhow!("Invalid CSR PEM: {}", e))?;
    let second = CertificateSigningRequestParams::from_pem(second_csr_pem)
        .map_err(|e| anyhow!("Invalid CSR PEM: {}", e))?;
    Ok(first.public_key.der_bytes() == second.public_key.der_bytes())
}

pub async fn csr_proves_agent_identity(
    db: &DatabaseConnection,
    agent_id: Uuid,
    csr_pem: &str,
) -> Result<bool> {
    let Some(active) = crate::db::certificates::get_active_certificate(db, agent_id)
        .await
        .map_err(|e| anyhow!("DB error: {}", e))?
    else {
        return Ok(false);
    };
    same_public_key(&active.public_key_pem, csr_pem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rcgen::{CertificateParams, KeyPair, PKCS_ECDSA_P256_SHA256};

    fn csr_for(key: &KeyPair) -> String {
        CertificateParams::new(vec!["agent.csfx.test".to_string()])
            .unwrap()
            .serialize_request(key)
            .unwrap()
            .pem()
            .unwrap()
    }

    #[test]
    fn csrs_are_matched_by_public_key_only() {
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        let other = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();

        assert!(same_public_key(&csr_for(&key), &csr_for(&key)).unwrap());
        assert!(!same_public_key(&csr_for(&key), &csr_for(&other)).unwrap());
        assert!(same_public_key(&csr_for(&key), "not a csr").is_err());
    }
}
