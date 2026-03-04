use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueCertificateRequest {
    pub csr_pem: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IssueCertificateResponse {
    pub certificate_pem: String,
    pub ca_cert_pem: String,
    pub serial_number: i64,
    pub expires_at: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RotateCertificateRequest {
    pub new_csr_pem: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct RevokeCertificateRequest {
    pub reason: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CrlResponse {
    pub revoked_serials: Vec<i64>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AgentEndpointResponse {
    pub agent_id: Uuid,
    pub hostname: String,
    pub ip_address: Option<String>,
    pub public_key_pem: Option<String>,
}
