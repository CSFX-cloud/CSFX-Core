use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const API_GATEWAY_URL_ENV: &str = "API_GATEWAY_URL";
const DEFAULT_API_GATEWAY_URL: &str = "http://localhost:8000";
const NOTIFY_TIMEOUT: Duration = Duration::from_secs(2);

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .timeout(NOTIFY_TIMEOUT)
            .build()
            .unwrap_or_default()
    })
}

pub async fn notify_assignment(agent_id: Uuid) {
    let base_url =
        std::env::var(API_GATEWAY_URL_ENV).unwrap_or_else(|_| DEFAULT_API_GATEWAY_URL.to_string());
    let url = format!(
        "{}/api/internal/agents/{}/notify-assignment",
        base_url, agent_id
    );

    if let Err(e) = http_client().post(&url).send().await {
        tracing::warn!(agent_id = %agent_id, error = %e, "failed to notify gateway of assignment");
    }
}
