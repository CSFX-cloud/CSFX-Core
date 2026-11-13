use std::time::{Duration, Instant};

const DEFAULT_FENCE_AFTER_SECS: u64 = 150;

pub struct FenceGuard {
    fence_after: Option<Duration>,
    last_contact: Instant,
    fenced: bool,
}

impl FenceGuard {
    pub fn from_env() -> Self {
        let secs = std::env::var("CSFX_SELF_FENCE_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_FENCE_AFTER_SECS);
        Self::new(secs, Instant::now())
    }

    pub fn new(fence_after_secs: u64, now: Instant) -> Self {
        Self {
            fence_after: (fence_after_secs > 0).then(|| Duration::from_secs(fence_after_secs)),
            last_contact: now,
            fenced: false,
        }
    }

    pub fn record_contact(&mut self, now: Instant) {
        self.last_contact = now;
        self.fenced = false;
    }

    pub fn record_isolation(&mut self, now: Instant) -> bool {
        let Some(limit) = self.fence_after else {
            return false;
        };
        if self.fenced || now.duration_since(self.last_contact) < limit {
            return false;
        }
        self.fenced = true;
        true
    }

    pub async fn should_fence(&mut self, error: &anyhow::Error) -> bool {
        if is_unreachable(error) && crate::isolation::isolated_from_peers().await {
            return self.record_isolation(Instant::now());
        }
        self.record_contact(Instant::now());
        false
    }
}

pub fn is_unreachable(error: &anyhow::Error) -> bool {
    error
        .chain()
        .filter_map(|cause| cause.downcast_ref::<reqwest::Error>())
        .any(|cause| cause.is_connect() || cause.is_timeout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_once_after_limit_since_last_contact() {
        let start = Instant::now();
        let mut guard = FenceGuard::new(60, start);
        assert!(!guard.record_isolation(start + Duration::from_secs(59)));
        assert!(guard.record_isolation(start + Duration::from_secs(60)));
        assert!(!guard.record_isolation(start + Duration::from_secs(90)));
    }

    #[test]
    fn contact_restarts_the_clock() {
        let start = Instant::now();
        let mut guard = FenceGuard::new(60, start);
        guard.record_contact(start + Duration::from_secs(100));
        assert!(!guard.record_isolation(start + Duration::from_secs(159)));
        assert!(guard.record_isolation(start + Duration::from_secs(160)));
    }

    #[test]
    fn zero_disables_fencing() {
        let start = Instant::now();
        let mut guard = FenceGuard::new(0, start);
        assert!(!guard.record_isolation(start + Duration::from_secs(10_000)));
    }

    #[tokio::test]
    async fn refused_connections_are_unreachable() {
        let error = reqwest::Client::new()
            .get("http://127.0.0.1:1")
            .send()
            .await
            .unwrap_err();
        assert!(is_unreachable(&anyhow::Error::new(error).context("send")));
    }

    #[test]
    fn http_status_failures_are_not_unreachable() {
        assert!(!is_unreachable(&anyhow::anyhow!(
            "Heartbeat failed status=503"
        )));
    }
}
