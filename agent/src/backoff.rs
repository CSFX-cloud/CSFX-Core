use ring::rand::{SecureRandom, SystemRandom};
use std::time::Duration;

const MAX_SHIFT: u32 = 30;
const RETRY_AFTER_JITTER_DIVISOR: u32 = 4;

pub fn ceiling(base: Duration, cap: Duration, attempt: u32) -> Duration {
    let factor = 1u32 << attempt.min(MAX_SHIFT);
    base.checked_mul(factor).unwrap_or(cap).min(cap)
}

fn random_unit() -> f64 {
    let mut bytes = [0u8; 8];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("system random source unavailable");
    (u64::from_le_bytes(bytes) >> 11) as f64 / (1u64 << 53) as f64
}

pub fn jitter(max: Duration) -> Duration {
    max.mul_f64(random_unit())
}

pub fn full_jitter(base: Duration, cap: Duration, attempt: u32) -> Duration {
    jitter(ceiling(base, cap, attempt))
}

pub fn equal_jitter(base: Duration, cap: Duration, attempt: u32) -> Duration {
    let half = ceiling(base, cap, attempt) / 2;
    half + jitter(half)
}

pub fn retry_after_delay(seconds: u64) -> Duration {
    let server = Duration::from_secs(seconds);
    server + jitter(server / RETRY_AFTER_JITTER_DIVISOR)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Duration = Duration::from_secs(1);
    const CAP: Duration = Duration::from_secs(60);

    #[test]
    fn ceiling_doubles_and_stops_at_the_cap() {
        let secs: Vec<u64> = (0..9).map(|n| ceiling(BASE, CAP, n).as_secs()).collect();
        assert_eq!(secs, vec![1, 2, 4, 8, 16, 32, 60, 60, 60]);
        assert_eq!(ceiling(BASE, CAP, u32::MAX), CAP);
    }

    #[test]
    fn jittered_delays_never_exceed_the_ceiling() {
        for attempt in 0..40 {
            let limit = ceiling(BASE, CAP, attempt);
            assert!(full_jitter(BASE, CAP, attempt) <= limit);
            let equal = equal_jitter(BASE, CAP, attempt);
            assert!(equal <= limit && equal >= limit / 2);
        }
    }

    #[test]
    fn thousand_reconnect_delays_spread_across_the_cap() {
        let delays: Vec<f64> = (0..1000)
            .map(|_| full_jitter(BASE, CAP, 20).as_secs_f64())
            .collect();
        let min = delays.iter().copied().fold(f64::MAX, f64::min);
        let max = delays.iter().copied().fold(0.0, f64::max);
        assert!(max - min >= 55.0, "spread {}", max - min);
        assert!(max < CAP.as_secs_f64());
        for bucket in 0..6 {
            let low = bucket as f64 * 10.0;
            assert!(delays.iter().any(|d| *d >= low && *d < low + 10.0));
        }
    }

    #[test]
    fn retry_after_is_honored_with_bounded_jitter() {
        assert_eq!(retry_after_delay(0), Duration::ZERO);
        for _ in 0..100 {
            let delay = retry_after_delay(40);
            assert!(delay >= Duration::from_secs(40) && delay <= Duration::from_secs(50));
        }
    }
}
