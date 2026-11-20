use reqwest::{Certificate, Client, ClientBuilder};
use std::time::Duration;

pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn builder(timeout: Duration) -> ClientBuilder {
    Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .timeout(timeout)
}

pub const CA_CERT_PATH_ENV: &str = "CSFX_CA_CERT_PATH";

fn load_cluster_ca(path: &str) -> Certificate {
    let pem = std::fs::read(path).expect("failed to read cluster ca");
    Certificate::from_pem(&pem).expect("failed to parse cluster ca")
}

pub fn internal_client(timeout: Duration) -> Client {
    let builder = builder(timeout);
    let builder = match std::env::var(CA_CERT_PATH_ENV) {
        Ok(path) => builder.tls_certs_only([load_cluster_ca(&path)]),
        Err(_) => builder,
    };
    builder
        .build()
        .expect("failed to build internal http client")
}

pub fn client(timeout: Duration) -> Client {
    builder(timeout)
        .build()
        .expect("failed to build http client")
}
