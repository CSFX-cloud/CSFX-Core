use anyhow::{Context, Result};
use rcgen::{CertificateParams, DnType, KeyPair, PKCS_ECDSA_P256_SHA256};
use std::path::Path;

const KEY_FILE: &str = "/var/lib/csfx-agent/agent.key";
const CSR_FILE: &str = "/var/lib/csfx-agent/agent.csr";
const CERT_FILE: &str = "/var/lib/csfx-agent/agent.crt";
const CA_FILE: &str = "/var/lib/csfx-agent/ca.crt";

pub struct AgentPki {
    key_pem: String,
    csr_pem: String,
}

impl AgentPki {
    pub fn load_or_generate() -> Result<Self> {
        if Path::new(KEY_FILE).exists() && Path::new(CSR_FILE).exists() {
            let key_pem = std::fs::read_to_string(KEY_FILE)
                .context("Failed to read agent key")?;
            let csr_pem = std::fs::read_to_string(CSR_FILE)
                .context("Failed to read agent CSR")?;

            tracing::info!("PKI: loaded existing keypair and CSR");
            return Ok(Self { key_pem, csr_pem });
        }

        Self::generate()
    }

    fn generate() -> Result<Self> {
        let key_pair = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .context("Failed to generate ECDSA keypair")?;

        let mut params = CertificateParams::default();
        params.distinguished_name.push(DnType::OrganizationName, "CS-Foundry");

        let csr = params
            .serialize_request(&key_pair)
            .context("Failed to serialize CSR")?;

        let key_pem = key_pair.serialize_pem();
        let csr_pem = csr.pem().context("Failed to encode CSR as PEM")?;

        std::fs::write(KEY_FILE, &key_pem).context("Failed to write agent key")?;
        set_permissions_600(KEY_FILE)?;
        std::fs::write(CSR_FILE, &csr_pem).context("Failed to write agent CSR")?;

        tracing::info!("PKI: generated new ECDSA keypair and CSR");

        Ok(Self { key_pem, csr_pem })
    }

    pub fn csr_pem(&self) -> &str {
        &self.csr_pem
    }

    pub fn key_pem(&self) -> &str {
        &self.key_pem
    }

    pub fn has_certificate() -> bool {
        Path::new(CERT_FILE).exists() && Path::new(CA_FILE).exists()
    }

    pub fn save_certificate(cert_pem: &str, ca_pem: &str) -> Result<()> {
        std::fs::write(CERT_FILE, cert_pem).context("Failed to write agent certificate")?;
        std::fs::write(CA_FILE, ca_pem).context("Failed to write CA certificate")?;
        tracing::info!("PKI: certificate and CA saved");
        Ok(())
    }

    pub fn load_cert_pem() -> Result<String> {
        std::fs::read_to_string(CERT_FILE).context("Failed to read agent certificate")
    }

    pub fn load_ca_pem() -> Result<String> {
        std::fs::read_to_string(CA_FILE).context("Failed to read CA certificate")
    }

    pub fn load_key_pem() -> Result<String> {
        std::fs::read_to_string(KEY_FILE).context("Failed to read agent key")
    }
}

#[cfg(unix)]
fn set_permissions_600(path: &str) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)
        .context("Failed to set file permissions")
}

#[cfg(not(unix))]
fn set_permissions_600(_path: &str) -> Result<()> {
    Ok(())
}
