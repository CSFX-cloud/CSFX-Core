use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams, DnType, IsCa, Issuer,
    KeyPair, KeyUsagePurpose, PKCS_ECDSA_P256_SHA256,
};
use sea_orm::DatabaseConnection;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;
use uuid::Uuid;

pub struct IssuedCertificate {
    pub certificate_pem: String,
    pub serial_number: i64,
    pub expires_at: DateTime<Utc>,
}

struct CaState {
    ca_cert_pem: String,
    ca_key_pem: String,
    serial_counter: AtomicI64,
}

pub struct PkiService {
    ca: Arc<CaState>,
    db: DatabaseConnection,
    cert_ttl_hours: i64,
}

impl PkiService {
    pub fn new(db: DatabaseConnection, cert_ttl_hours: i64) -> Result<Self> {
        let ca = Self::load_or_generate_ca()?;
        Ok(Self {
            ca: Arc::new(ca),
            db,
            cert_ttl_hours,
        })
    }

    fn load_or_generate_ca() -> Result<CaState> {
        match (
            std::env::var("CSFX_CA_CERT_PEM"),
            std::env::var("CSFX_CA_KEY_PEM"),
        ) {
            (Ok(cert_pem), Ok(key_pem)) => {
                KeyPair::from_pem(&key_pem).map_err(|e| anyhow!("Failed to load CA key: {}", e))?;

                crate::log_info!("pki", "CA loaded from environment");

                Ok(CaState {
                    ca_cert_pem: cert_pem,
                    ca_key_pem: key_pem,
                    serial_counter: AtomicI64::new(1),
                })
            }
            _ => {
                crate::log_warn!(
                    "pki",
                    "CSFX_CA_CERT_PEM/CSFX_CA_KEY_PEM not set, generating ephemeral CA"
                );
                Self::generate_ca()
            }
        }
    }

    fn generate_ca() -> Result<CaState> {
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| anyhow!("Failed to generate CA key: {}", e))?;

        let mut params = CertificateParams::default();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params
            .distinguished_name
            .push(DnType::CommonName, "CSFX Internal CA");
        params
            .distinguished_name
            .push(DnType::OrganizationName, "CSFX");
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign, KeyUsagePurpose::CrlSign];
        params.not_before = rcgen::date_time_ymd(2024, 1, 1);
        params.not_after = rcgen::date_time_ymd(2035, 1, 1);

        let cert = params
            .self_signed(&key_pair)
            .map_err(|e| anyhow!("Failed to self-sign CA: {}", e))?;

        let ca_cert_pem = cert.pem();
        let ca_key_pem = key_pair.serialize_pem();

        crate::log_info!("pki", "Ephemeral CA generated");
        crate::log_info!("pki", &format!("CA cert PEM:\n{}", ca_cert_pem));

        Ok(CaState {
            ca_cert_pem,
            ca_key_pem,
            serial_counter: AtomicI64::new(1),
        })
    }

    fn build_issuer(&self) -> Result<Issuer<'_, KeyPair>> {
        let key_pair = KeyPair::from_pem(&self.ca.ca_key_pem)
            .map_err(|e| anyhow!("Failed to load CA key: {}", e))?;

        Issuer::from_ca_cert_pem(&self.ca.ca_cert_pem, key_pair)
            .map_err(|e| anyhow!("Failed to build issuer: {}", e))
    }

    pub fn ca_cert_pem(&self) -> String {
        self.ca.ca_cert_pem.clone()
    }

    pub async fn issue_certificate(
        &self,
        agent_id: Uuid,
        csr_pem: &str,
    ) -> Result<IssuedCertificate> {
        let serial = self.ca.serial_counter.fetch_add(1, Ordering::SeqCst);
        let expires_at = Utc::now() + chrono::Duration::hours(self.cert_ttl_hours);

        let csr_params = CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| anyhow!("Invalid CSR PEM: {}", e))?;

        let issuer = self.build_issuer()?;

        let cert = csr_params
            .signed_by(&issuer)
            .map_err(|e| anyhow!("Failed to sign certificate: {}", e))?;

        let certificate_pem = cert.pem();

        let public_key_pem = csr_pem.to_string();

        crate::db::certificates::create_certificate(
            &self.db,
            agent_id,
            serial,
            certificate_pem.clone(),
            public_key_pem,
            expires_at,
        )
        .await
        .map_err(|e| anyhow!("Failed to store certificate: {}", e))?;

        crate::log_info!(
            "pki",
            &format!("Issued certificate agent={} serial={}", agent_id, serial)
        );

        Ok(IssuedCertificate {
            certificate_pem,
            serial_number: serial,
            expires_at,
        })
    }

    pub async fn rotate_certificate(
        &self,
        agent_id: Uuid,
        new_csr_pem: &str,
    ) -> Result<IssuedCertificate> {
        if let Some(old_cert) = crate::db::certificates::get_active_certificate(&self.db, agent_id)
            .await
            .map_err(|e| anyhow!("DB error: {}", e))?
        {
            crate::db::certificates::revoke_certificate(
                &self.db,
                old_cert.serial_number,
                agent_id,
                "rotation".to_string(),
            )
            .await
            .map_err(|e| anyhow!("Failed to revoke old cert: {}", e))?;

            crate::log_info!(
                "pki",
                &format!(
                    "Revoked old certificate agent={} serial={}",
                    agent_id, old_cert.serial_number
                )
            );
        }

        self.issue_certificate(agent_id, new_csr_pem).await
    }

    pub async fn revoke_agent_certificate(&self, agent_id: Uuid, reason: String) -> Result<()> {
        let cert = crate::db::certificates::get_active_certificate(&self.db, agent_id)
            .await
            .map_err(|e| anyhow!("DB error: {}", e))?
            .ok_or_else(|| anyhow!("No active certificate for agent: {}", agent_id))?;

        crate::db::certificates::revoke_certificate(&self.db, cert.serial_number, agent_id, reason)
            .await
            .map_err(|e| anyhow!("Failed to revoke certificate: {}", e))?;

        crate::log_info!(
            "pki",
            &format!(
                "Revoked certificate agent={} serial={}",
                agent_id, cert.serial_number
            )
        );

        Ok(())
    }

    pub async fn build_crl(&self) -> Result<Vec<i64>> {
        crate::db::certificates::get_revoked_serials(&self.db)
            .await
            .map_err(|e| anyhow!("Failed to build CRL: {}", e))
    }
}
