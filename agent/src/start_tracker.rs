use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{Mutex, Notify, OwnedSemaphorePermit, Semaphore};

use crate::{backoff, client};

const DEFAULT_MAX_PARALLEL_STARTS: usize = 4;
const START_RETRY_BASE: Duration = Duration::from_secs(5);
const START_RETRY_CAP: Duration = Duration::from_secs(300);

#[derive(Clone)]
pub struct StartTracker {
    in_flight: Arc<Mutex<HashSet<String>>>,
    retries: Arc<Mutex<HashMap<String, (u32, Instant)>>>,
    limiter: Arc<Semaphore>,
    pub completed: Arc<Notify>,
}

impl Default for StartTracker {
    fn default() -> Self {
        Self::new(DEFAULT_MAX_PARALLEL_STARTS)
    }
}

impl StartTracker {
    pub fn new(max_parallel: usize) -> Self {
        Self {
            in_flight: Arc::default(),
            retries: Arc::default(),
            limiter: Arc::new(Semaphore::new(max_parallel.max(1))),
            completed: Arc::default(),
        }
    }

    pub fn from_env() -> Self {
        let max_parallel = std::env::var("CSFX_MAX_PARALLEL_STARTS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(DEFAULT_MAX_PARALLEL_STARTS);
        Self::new(max_parallel)
    }

    pub async fn permit(&self) -> OwnedSemaphorePermit {
        Arc::clone(&self.limiter)
            .acquire_owned()
            .await
            .expect("start semaphore is never closed")
    }

    pub async fn try_begin(&self, workload_id: &str) -> bool {
        self.try_begin_at(workload_id, Instant::now()).await
    }

    async fn try_begin_at(&self, workload_id: &str, now: Instant) -> bool {
        let backing_off = self
            .retries
            .lock()
            .await
            .get(workload_id)
            .is_some_and(|(_, next_attempt)| now < *next_attempt);
        !backing_off && self.in_flight.lock().await.insert(workload_id.to_string())
    }

    async fn record_failure(&self, workload_id: &str, now: Instant) {
        let mut retries = self.retries.lock().await;
        let failures = retries.get(workload_id).map_or(0, |(count, _)| *count);
        let delay = backoff::equal_jitter(START_RETRY_BASE, START_RETRY_CAP, failures);
        retries.insert(workload_id.to_string(), (failures + 1, now + delay));
    }

    pub async fn forget_unassigned(&self, workloads: &[client::AssignedWorkload]) {
        let assigned: HashSet<&str> = workloads.iter().map(|w| w.id.as_str()).collect();
        self.retries
            .lock()
            .await
            .retain(|id, _| assigned.contains(id.as_str()));
    }

    pub async fn finish(&self, workload_id: &str, state_changed: bool, failed: bool) {
        self.in_flight.lock().await.remove(workload_id);
        if failed {
            self.record_failure(workload_id, Instant::now()).await;
        } else {
            self.retries.lock().await.remove(workload_id);
        }
        if state_changed {
            self.completed.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn start_tracker_admits_one_start_per_workload() {
        let tracker = StartTracker::default();
        assert!(tracker.try_begin("w").await);
        assert!(!tracker.try_begin("w").await);
        tracker.finish("w", true, false).await;
        tokio::time::timeout(Duration::from_secs(1), tracker.completed.notified())
            .await
            .unwrap();
        assert!(tracker.try_begin("w").await);
        tracker.finish("w", false, false).await;
    }

    #[tokio::test]
    async fn start_tracker_limits_parallel_starts() {
        use std::sync::atomic::{AtomicUsize, Ordering};

        let tracker = StartTracker::new(2);
        let running = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let tasks: Vec<_> = (0..8)
            .map(|_| {
                let (tracker, running, peak) = (tracker.clone(), running.clone(), peak.clone());
                tokio::spawn(async move {
                    let _permit = tracker.permit().await;
                    let now = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    running.fetch_sub(1, Ordering::SeqCst);
                })
            })
            .collect();
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(peak.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn failed_starts_back_off_exponentially_per_workload() {
        let tracker = StartTracker::default();
        let origin = Instant::now();
        let mut previous_ceiling = Duration::ZERO;
        for failures in 0..5u32 {
            tracker.record_failure("w", origin).await;
            let ceiling = backoff::ceiling(START_RETRY_BASE, START_RETRY_CAP, failures);
            assert!(ceiling > previous_ceiling);
            previous_ceiling = ceiling;
            assert!(
                !tracker
                    .try_begin_at("w", origin + ceiling / 2 - Duration::from_millis(1))
                    .await
            );
            assert!(tracker.try_begin_at("w", origin + ceiling).await);
            tracker.in_flight.lock().await.remove("w");
        }
        assert!(tracker.try_begin_at("other", origin).await);
        tracker.finish("w", false, false).await;
        assert!(tracker.try_begin_at("w", origin).await);
    }
}
