use shared::internal_auth::SignedRequest;
use std::sync::OnceLock;
use std::time::Duration;
use uuid::Uuid;

const NOTIFY_TIMEOUT: Duration = Duration::from_secs(2);

fn http_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| shared::http::internal_client(NOTIFY_TIMEOUT))
}

pub async fn notify_assignment(agent_id: Uuid) {
    let base_url = shared::internal_auth::gateway_internal_url();
    let url = format!(
        "{}/internal/agents/{}/notify-assignment",
        base_url, agent_id
    );

    let result = http_client()
        .post(&url)
        .signed()
        .send()
        .await
        .and_then(|response| response.error_for_status());

    if let Err(e) = result {
        crate::log_warn!(
            "gateway_notify",
            &format!(
                "Failed to notify gateway of assignment agent_id={} url={} err={}",
                agent_id, url, e
            )
        );
    }
}
