use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

pub async fn wait_for_gateway(
    gateway_url: &str,
    timeout_secs: u64,
    retry_interval_secs: u64,
) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);
    let probe_url = format!("{}/api/public-key", gateway_url.trim_end_matches('/'));
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(5))
        .danger_accept_invalid_certs(true)
        .build()
        .expect("failed to build http client");

    loop {
        match client.get(&probe_url).send().await {
            Ok(resp) if resp.status().is_success() || resp.status().as_u16() == 401 => {
                info!(url = %probe_url, "api-gateway health check passed");
                return true;
            }
            Ok(resp) => {
                warn!(url = %probe_url, status = %resp.status(), "api-gateway returned unexpected status");
            }
            Err(e) => {
                warn!(url = %probe_url, error = %e, "api-gateway not reachable");
            }
        }

        if tokio::time::Instant::now() >= deadline {
            return false;
        }

        sleep(Duration::from_secs(retry_interval_secs)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn serve_status(status: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            loop {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let response =
                    format!("HTTP/1.1 {status}\r\ncontent-length: 0\r\nconnection: close\r\n\r\n");
                let _ = stream.write_all(response.as_bytes()).await;
            }
        });
        format!("http://{address}/")
    }

    fn install_crypto_provider() {
        rustls::crypto::ring::default_provider()
            .install_default()
            .ok();
    }

    #[tokio::test]
    async fn success_and_unauthorized_count_as_healthy() {
        install_crypto_provider();
        assert!(wait_for_gateway(&serve_status("200 OK").await, 2, 1).await);
        assert!(wait_for_gateway(&serve_status("401 Unauthorized").await, 2, 1).await);
    }

    #[tokio::test]
    async fn persistent_server_errors_exhaust_the_deadline() {
        install_crypto_provider();
        let url = serve_status("503 Service Unavailable").await;
        assert!(!wait_for_gateway(&url, 0, 1).await);
    }

    #[tokio::test]
    async fn unreachable_gateways_exhaust_the_deadline() {
        install_crypto_provider();
        assert!(!wait_for_gateway("http://127.0.0.1:1", 0, 1).await);
    }
}
