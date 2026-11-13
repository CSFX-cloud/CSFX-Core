use tokio::time::{Duration, Instant};

const RECOVERY_GRACE: Duration = Duration::from_secs(180);
const STALL_LIMIT: Duration = Duration::from_secs(45);

#[derive(Default)]
pub struct DatabaseHealth {
    stable_since: Option<Instant>,
    last_success: Option<Instant>,
    failed: bool,
}

impl DatabaseHealth {
    pub fn record_interruption(&mut self) {
        self.failed = true;
        self.stable_since = None;
        self.last_success = None;
    }

    pub fn record_success(&mut self, now: Instant) {
        match self.last_success {
            None if !self.failed => {
                self.stable_since = Some(now.checked_sub(RECOVERY_GRACE).unwrap_or(now));
            }
            Some(last) if now.duration_since(last) <= STALL_LIMIT => {}
            _ => self.stable_since = Some(now),
        }
        self.last_success = Some(now);
    }

    pub fn settled(&self, now: Instant) -> bool {
        self.stable_since
            .is_some_and(|since| now.duration_since(since) >= RECOVERY_GRACE)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(start: Instant, secs: u64) -> Instant {
        start + Duration::from_secs(secs)
    }

    #[test]
    fn a_healthy_start_is_settled_immediately() {
        let start = Instant::now() + RECOVERY_GRACE;
        let mut health = DatabaseHealth::default();
        health.record_success(start);
        assert!(health.settled(start));
    }

    #[test]
    fn regular_successes_stay_settled() {
        let start = Instant::now() + RECOVERY_GRACE;
        let mut health = DatabaseHealth::default();
        health.record_success(start);
        health.record_success(at(start, 15));
        health.record_success(at(start, 30));
        assert!(health.settled(at(start, 30)));
    }

    #[test]
    fn a_failure_defers_until_the_grace_has_passed() {
        let start = Instant::now() + RECOVERY_GRACE;
        let mut health = DatabaseHealth::default();
        health.record_success(start);
        health.record_interruption();
        health.record_success(at(start, 100));
        assert!(!health.settled(at(start, 100)));
        health.record_success(at(start, 115));
        assert!(!health.settled(at(start, 279)));
        assert!(health.settled(at(start, 280)));
    }

    #[test]
    fn a_stalled_cycle_is_treated_as_an_outage() {
        let start = Instant::now() + RECOVERY_GRACE;
        let mut health = DatabaseHealth::default();
        health.record_success(start);
        health.record_success(at(start, 76));
        assert!(!health.settled(at(start, 76)));
        assert!(health.settled(at(start, 256)));
    }

    #[test]
    fn a_failure_before_any_success_defers() {
        let start = Instant::now() + RECOVERY_GRACE;
        let mut health = DatabaseHealth::default();
        health.record_interruption();
        health.record_success(start);
        assert!(!health.settled(start));
    }
}
