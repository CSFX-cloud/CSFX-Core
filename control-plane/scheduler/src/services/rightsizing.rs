const HEADROOM: f64 = 1.2;
const MILLICORES_PER_PERCENT: f64 = 10.0;
const MIN_CPU_MILLICORES: i32 = 100;
const MIN_MEMORY_BYTES: i64 = 128 * 1024 * 1024;

pub fn recommend_cpu_millicores(peak_usage_percent: f64) -> i32 {
    let millicores = (peak_usage_percent * MILLICORES_PER_PERCENT * HEADROOM).ceil() as i32;
    millicores.max(MIN_CPU_MILLICORES)
}

pub fn recommend_memory_bytes(peak_usage_bytes: i64) -> i64 {
    ((peak_usage_bytes as f64 * HEADROOM) as i64).max(MIN_MEMORY_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpu_recommendation_adds_headroom_and_respects_floor() {
        assert_eq!(recommend_cpu_millicores(50.0), 600);
        assert_eq!(recommend_cpu_millicores(1.0), MIN_CPU_MILLICORES);
    }

    #[test]
    fn memory_recommendation_adds_headroom_and_respects_floor() {
        assert_eq!(recommend_memory_bytes(1_000_000_000), 1_200_000_000);
        assert_eq!(recommend_memory_bytes(1), MIN_MEMORY_BYTES);
    }
}
