use axum_server::tls_rustls::RustlsConfig;
use rcgen::{CertificateParams, DistinguishedName, DnType, Issuer, KeyPair, SanType};
use std::sync::Arc;

const CA_CERT_PATH: &str = "/var/lib/csfx-cp/ca.crt";
const CA_KEY_PATH: &str = "/var/lib/csfx-cp/ca.key";
const CA_DISK_WAIT_ATTEMPTS: u32 = 20;
const CA_DISK_WAIT_INTERVAL: std::time::Duration = std::time::Duration::from_millis(250);

fn read_ca_from_disk() -> Option<(String, String)> {
    std::fs::read_to_string(CA_CERT_PATH)
        .ok()
        .zip(std::fs::read_to_string(CA_KEY_PATH).ok())
}

pub async fn load_ca_pem() -> Option<(String, String)> {
    if let (Ok(cert), Ok(key)) = (
        std::env::var("CSFX_CA_CERT_PEM"),
        std::env::var("CSFX_CA_KEY_PEM"),
    ) {
        return Some((cert, key));
    }
    for attempt in 0..CA_DISK_WAIT_ATTEMPTS {
        if let Some(pair) = read_ca_from_disk() {
            return Some(pair);
        }
        if attempt + 1 < CA_DISK_WAIT_ATTEMPTS {
            tokio::time::sleep(CA_DISK_WAIT_INTERVAL).await;
        }
    }
    None
}

fn detect_primary_ip() -> Option<std::net::IpAddr> {
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    Some(socket.local_addr().ok()?.ip())
}

fn collect_sans() -> anyhow::Result<(String, Vec<SanType>)> {
    let mut san_hosts =
        std::env::var("TLS_SANS").unwrap_or_else(|_| "localhost,127.0.0.1".to_string());
    if let Some(primary_ip) = detect_primary_ip() {
        let primary_ip = primary_ip.to_string();
        if !san_hosts.split(',').any(|s| s.trim() == primary_ip) {
            san_hosts.push(',');
            san_hosts.push_str(&primary_ip);
        }
    }

    let mut sans = Vec::new();
    for san in san_hosts.split(',') {
        let san = san.trim();
        if let Ok(ip) = san.parse::<std::net::IpAddr>() {
            sans.push(SanType::IpAddress(ip));
        } else {
            sans.push(SanType::DnsName(san.try_into()?));
        }
    }

    Ok((san_hosts, sans))
}

fn restrict_alpn_to_http1(config: &RustlsConfig) {
    let mut inner = (*config.get_inner()).clone();
    inner.alpn_protocols = vec![b"http/1.1".to_vec()];
    config.reload_from_config(Arc::new(inner));
}

pub async fn generate_tls_config() -> anyhow::Result<RustlsConfig> {
    let cert_path = std::env::var("TLS_CERT").unwrap_or_default();
    let key_path = std::env::var("TLS_KEY").unwrap_or_default();

    if !cert_path.is_empty() && !key_path.is_empty() {
        let cert_pem = std::fs::read(&cert_path)?;
        let key_pem = std::fs::read(&key_path)?;
        let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;
        restrict_alpn_to_http1(&config);
        tracing::info!(cert = %cert_path, "TLS loaded from files");
        return Ok(config);
    }

    let key_pair = KeyPair::generate()?;

    let mut params = CertificateParams::default();
    params.distinguished_name = DistinguishedName::new();
    params
        .distinguished_name
        .push(DnType::CommonName, "csfx-gateway");

    let (san_hosts, sans) = collect_sans()?;
    params.subject_alt_names = sans;

    match load_ca_pem().await {
        Some((ca_cert_pem, ca_key_pem)) => {
            let ca_key_pair = KeyPair::from_pem(&ca_key_pem)?;
            let issuer = Issuer::from_ca_cert_pem(&ca_cert_pem, ca_key_pair)?;
            let cert = params.signed_by(&key_pair, &issuer)?;
            let cert_pem = cert.pem().into_bytes();
            let key_pem = key_pair.serialize_pem().into_bytes();

            let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;
            restrict_alpn_to_http1(&config);

            tracing::info!(sans = %san_hosts, "TLS certificate signed by internal CA");
            Ok(config)
        }
        None => {
            let cert = params.self_signed(&key_pair)?;
            let cert_pem = cert.pem().into_bytes();
            let key_pem = key_pair.serialize_pem().into_bytes();

            let config = RustlsConfig::from_pem(cert_pem, key_pem).await?;
            restrict_alpn_to_http1(&config);

            tracing::warn!(
                sans = %san_hosts,
                "no ca found in env or on disk, using self-signed TLS certificate"
            );
            Ok(config)
        }
    }
}
