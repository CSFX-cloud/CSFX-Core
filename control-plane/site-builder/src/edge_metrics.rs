use crate::encoding::Encoding;
use prometheus::{register_int_counter_vec, IntCounterVec};
use std::sync::LazyLock;

static CACHE_LOOKUPS: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "csfx_site_edge_cache_lookups_total",
        "Site edge cache lookups by cache and result",
        &["cache", "result"]
    )
    .expect("failed to register csfx_site_edge_cache_lookups_total")
});

static CONTENT_BYTES: LazyLock<IntCounterVec> = LazyLock::new(|| {
    register_int_counter_vec!(
        "csfx_site_edge_content_bytes_total",
        "Site edge response bytes before and after compression",
        &["stage", "encoding"]
    )
    .expect("failed to register csfx_site_edge_content_bytes_total")
});

pub fn cache_lookup(cache: &'static str, hit: bool) {
    let result = if hit { "hit" } else { "miss" };
    CACHE_LOOKUPS.with_label_values(&[cache, result]).inc();
}

pub fn content_served(encoding: Encoding, original: usize, sent: usize) {
    let label = encoding.token().unwrap_or("identity");
    CONTENT_BYTES
        .with_label_values(&["original", label])
        .inc_by(original as u64);
    CONTENT_BYTES
        .with_label_values(&["sent", label])
        .inc_by(sent as u64);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookups(cache: &str, result: &str) -> u64 {
        CACHE_LOOKUPS.with_label_values(&[cache, result]).get()
    }

    #[test]
    fn lookups_are_counted_per_result() {
        let hits = lookups("probe", "hit");
        let misses = lookups("probe", "miss");
        cache_lookup("probe", true);
        cache_lookup("probe", true);
        cache_lookup("probe", false);
        assert_eq!(lookups("probe", "hit"), hits + 2);
        assert_eq!(lookups("probe", "miss"), misses + 1);
    }

    #[test]
    fn served_bytes_are_split_by_stage_and_encoding() {
        let sent = CONTENT_BYTES.with_label_values(&["sent", "br"]).get();
        content_served(Encoding::Brotli, 1000, 200);
        assert_eq!(
            CONTENT_BYTES.with_label_values(&["sent", "br"]).get(),
            sent + 200
        );
    }
}
