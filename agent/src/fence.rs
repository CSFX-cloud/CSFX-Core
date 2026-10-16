use std::time::{Duration, Instant};

const DEFAULT_FENCE_AFTER_SECS: u64 = 150;

pub struct FenceGuard {
    fence_after: Option<Duration>,
    isolated_since: Option<Instant>,
    fenced: bool,
}

impl FenceGuard {
    pub fn from_env() -> Self {
        let secs = std::env::var("CSFX_SELF_FENCE_SECS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_FENCE_AFTER_SECS);
        Self::new(secs)
    }

    pub fn new(fence_after_secs: u64) -> Self {
        Self {
            fence_after: (fence_after_secs > 0).then(|| Duration::from_secs(fence_after_secs)),
            isolated_since: None,
            fenced: false,
        }
    }

    pub fn record_success(&mut self) {
        self.isolated_since = None;
        self.fenced = false;
    }

    pub fn record_failure(&mut self, now: Instant) -> bool {
        let since = *self.isolated_since.get_or_insert(now);
        let Some(limit) = self.fence_after else {
            return false;
        };
        if self.fenced || now.duration_since(since) < limit {
            return false;
        }
        self.fenced = true;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fences_once_after_limit() {
        let start = Instant::now();
        let mut guard = FenceGuard::new(60);
        assert!(!guard.record_failure(start));
        assert!(!guard.record_failure(start + Duration::from_secs(59)));
        assert!(guard.record_failure(start + Duration::from_secs(60)));
        assert!(!guard.record_failure(start + Duration::from_secs(90)));
    }

    #[test]
    fn success_resets_isolation() {
        let start = Instant::now();
        let mut guard = FenceGuard::new(60);
        assert!(!guard.record_failure(start));
        guard.record_success();
        assert!(!guard.record_failure(start + Duration::from_secs(100)));
        assert!(guard.record_failure(start + Duration::from_secs(160)));
    }

    #[test]
    fn zero_disables_fencing() {
        let start = Instant::now();
        let mut guard = FenceGuard::new(0);
        assert!(!guard.record_failure(start));
        assert!(!guard.record_failure(start + Duration::from_secs(10_000)));
    }
}
