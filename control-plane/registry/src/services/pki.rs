use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Utc};
use rcgen::{
    BasicConstraints, CertificateParams, CertificateSigningRequestParams, DnType,
    ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair, KeyUsagePurpose, SanType, SerialNumber,
    SigningKey, PKCS_ECDSA_P256_SHA256,
};
use sea_orm::DatabaseConnection;
use std::sync::Arc;
use time::OffsetDateTime;
use uuid::Uuid;

pub struct IssuedCertificate {
    pub certificate_pem: String,
    pub serial_number: i64,
    pub expires_at: DateTime<Utc>,
}

pub struct ProxyTicket {
    pub payload: String,
    pub signature: String,
}

const PROXY_TICKET_TTL_SECS: i64 = 60;
const CA_STATE_DIR: &str = "/var/lib/csfx-cp";
const CA_CERT_PATH: &str = "/var/lib/csfx-cp/ca.crt";
const CA_KEY_PATH: &str = "/var/lib/csfx-cp/ca.key";

struct CaState {
    ca_cert_pem: String,
    ca_key_pem: String,
}

fn random_serial() -> i64 {
    (Uuid::new_v4().as_u128() >> 65) as i64
}

fn agent_certificate_params(
    agent_id: Uuid,
    serial: i64,
    expires_at: DateTime<Utc>,
) -> Result<CertificateParams> {
    let identity = agent_id.to_string();
    let mut params = CertificateParams::default();
    params
        .distinguished_name
        .push(DnType::CommonName, &identity);
    params.subject_alt_names = vec![SanType::DnsName(
        identity
            .try_into()
            .map_err(|e| anyhow!("Invalid agent identity: {}", e))?,
    )];
    params.is_ca = IsCa::ExplicitNoCa;
    params.key_usages = vec![KeyUsagePurpose::DigitalSignature];
    params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ClientAuth];
    params.serial_number = Some(SerialNumber::from(serial as u64));
    params.not_before = OffsetDateTime::now_utc();
    params.not_after = OffsetDateTime::from_unix_timestamp(expires_at.timestamp())
        .map_err(|e| anyhow!("Invalid certificate expiry: {}", e))?;
    Ok(params)
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
        if let (Ok(cert_pem), Ok(key_pem)) = (
            std::env::var("CSFX_CA_CERT_PEM"),
            std::env::var("CSFX_CA_KEY_PEM"),
        ) {
            KeyPair::from_pem(&key_pem).map_err(|e| anyhow!("Failed to load CA key: {}", e))?;
            crate::log_info!("pki", "CA loaded from environment");
            return Ok(CaState {
                ca_cert_pem: cert_pem,
                ca_key_pem: key_pem,
            });
        }

        if let (Ok(cert_pem), Ok(key_pem)) = (
            std::fs::read_to_string(CA_CERT_PATH),
            std::fs::read_to_string(CA_KEY_PATH),
        ) {
            KeyPair::from_pem(&key_pem).map_err(|e| anyhow!("Failed to load CA key: {}", e))?;
            crate::log_info!("pki", "CA loaded from disk");
            return Ok(CaState {
                ca_cert_pem: cert_pem,
                ca_key_pem: key_pem,
            });
        }

        crate::log_warn!(
            "pki",
            "no persisted CA found, generating and persisting new CA"
        );
        let ca = Self::generate_ca()?;
        std::fs::create_dir_all(CA_STATE_DIR)
            .map_err(|e| anyhow!("Failed to create CA state dir: {}", e))?;
        std::fs::write(CA_CERT_PATH, &ca.ca_cert_pem)
            .map_err(|e| anyhow!("Failed to persist CA cert: {}", e))?;
        std::fs::write(CA_KEY_PATH, &ca.ca_key_pem)
            .map_err(|e| anyhow!("Failed to persist CA key: {}", e))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(CA_KEY_PATH, std::fs::Permissions::from_mode(0o600))
                .map_err(|e| anyhow!("Failed to set CA key permissions: {}", e))?;
        }
        crate::log_info!("pki", "CA generated and persisted to disk");
        Ok(ca)
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

        crate::log_info!("pki", "CA key pair generated");

        Ok(CaState {
            ca_cert_pem,
            ca_key_pem,
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
        let serial = random_serial();
        let expires_at = Utc::now() + chrono::Duration::hours(self.cert_ttl_hours);

        let csr_params = CertificateSigningRequestParams::from_pem(csr_pem)
            .map_err(|e| anyhow!("Invalid CSR PEM: {}", e))?;

        let issuer = self.build_issuer()?;

        let cert = agent_certificate_params(agent_id, serial, expires_at)?
            .signed_by(&csr_params.public_key, &issuer)
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

    pub async fn revoke_active_certificate(&self, agent_id: Uuid, reason: String) -> Result<()> {
        let active = crate::db::certificates::get_active_certificate(&self.db, agent_id)
            .await
            .map_err(|e| anyhow!("DB error: {}", e))?;
        if active.is_none() {
            return Ok(());
        }
        self.revoke_agent_certificate(agent_id, reason).await
    }

    pub async fn build_crl(&self) -> Result<Vec<i64>> {
        crate::db::certificates::get_revoked_serials(&self.db)
            .await
            .map_err(|e| anyhow!("Failed to build CRL: {}", e))
    }

    pub fn sign_proxy_ticket(&self, agent_id: Uuid, workload_id: &str) -> Result<ProxyTicket> {
        let expires_at = Utc::now() + chrono::Duration::seconds(PROXY_TICKET_TTL_SECS);
        let payload = format!("{}.{}.{}", agent_id, workload_id, expires_at.timestamp());

        let key_pair = KeyPair::from_pem(&self.ca.ca_key_pem)
            .map_err(|e| anyhow!("Failed to load CA key: {}", e))?;

        let signature = key_pair
            .sign(payload.as_bytes())
            .map_err(|e| anyhow!("Failed to sign proxy ticket: {}", e))?;

        Ok(ProxyTicket {
            payload: URL_SAFE_NO_PAD.encode(payload.as_bytes()),
            signature: URL_SAFE_NO_PAD.encode(signature),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use x509_parser::{extensions::ParsedExtension, parse_x509_certificate, pem::parse_x509_pem};

    fn hostile_csr() -> String {
        let mut params = CertificateParams::new(vec!["csfx-gateway".to_string()]).unwrap();
        params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        params.key_usages = vec![KeyUsagePurpose::KeyCertSign];
        params.extended_key_usages = vec![ExtendedKeyUsagePurpose::ServerAuth];
        let key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256).unwrap();
        params.serialize_request(&key).unwrap().pem().unwrap()
    }

    fn issue(agent_id: Uuid, ttl_hours: i64) -> (String, DateTime<Utc>) {
        let ca = PkiService::generate_ca().unwrap();
        let ca_key = KeyPair::from_pem(&ca.ca_key_pem).unwrap();
        let issuer = Issuer::from_ca_cert_pem(&ca.ca_cert_pem, ca_key).unwrap();
        let csr = CertificateSigningRequestParams::from_pem(&hostile_csr()).unwrap();
        let expires_at = Utc::now() + chrono::Duration::hours(ttl_hours);
        let cert = agent_certificate_params(agent_id, 42, expires_at)
            .unwrap()
            .signed_by(&csr.public_key, &issuer)
            .unwrap();
        (cert.pem(), expires_at)
    }

    #[test]
    fn issued_certificate_ignores_csr_requested_privileges() {
        let agent_id = Uuid::new_v4();
        let (pem, expires_at) = issue(agent_id, 24);
        let (_, pem) = parse_x509_pem(pem.as_bytes()).unwrap();
        let (_, cert) = parse_x509_certificate(&pem.contents).unwrap();

        assert!(!cert.is_ca());
        assert_eq!(
            cert.validity().not_after.timestamp(),
            expires_at.timestamp()
        );
        assert!(
            cert.validity().not_after.timestamp() - cert.validity().not_before.timestamp()
                <= 24 * 3600 + 1
        );
        assert_eq!(
            cert.subject()
                .iter_common_name()
                .next()
                .unwrap()
                .as_str()
                .unwrap(),
            agent_id.to_string()
        );

        let mut sans = Vec::new();
        let mut client_auth_only = false;
        let mut digital_signature_only = false;
        for extension in cert.extensions() {
            match extension.parsed_extension() {
                ParsedExtension::SubjectAlternativeName(san) => {
                    sans = san
                        .general_names
                        .iter()
                        .map(|name| name.to_string())
                        .collect();
                }
                ParsedExtension::ExtendedKeyUsage(eku) => {
                    client_auth_only = eku.client_auth
                        && !eku.server_auth
                        && !eku.any
                        && !eku.code_signing
                        && !eku.email_protection
                        && !eku.time_stamping
                        && !eku.ocsp_signing
                        && eku.other.is_empty();
                }
                ParsedExtension::KeyUsage(usage) => {
                    digital_signature_only =
                        usage.digital_signature() && !usage.key_cert_sign() && !usage.crl_sign();
                }
                _ => {}
            }
        }
        assert_eq!(sans, vec![format!("DNSName({agent_id})")]);
        assert!(client_auth_only);
        assert!(digital_signature_only);
    }
}
