use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct TtlCache<V> {
    ttl: Duration,
    entries: Mutex<HashMap<String, (Instant, V)>>,
}

impl<V: Clone> TtlCache<V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            ttl,
            entries: Mutex::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let (stored_at, value) = entries.get(key)?;
        (stored_at.elapsed() < self.ttl).then(|| value.clone())
    }

    pub fn remove(&self, key: &str) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(key);
    }

    pub fn insert_if_absent(&self, key: &str, value: V) -> bool {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let ttl = self.ttl;
        entries.retain(|_, (stored_at, _)| stored_at.elapsed() < ttl);
        if entries.contains_key(key) {
            return false;
        }
        entries.insert(key.to_string(), (Instant::now(), value));
        true
    }

    pub fn clear(&self) {
        self.entries
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }

    pub fn insert(&self, key: &str, value: V) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        let ttl = self.ttl;
        entries.retain(|_, (stored_at, _)| stored_at.elapsed() < ttl);
        entries.insert(key.to_string(), (Instant::now(), value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_fresh_entries_only() {
        let cache = TtlCache::new(Duration::from_millis(40));
        assert_eq!(cache.get("a"), None);
        cache.insert("a", 1);
        assert_eq!(cache.get("a"), Some(1));
        std::thread::sleep(Duration::from_millis(60));
        assert_eq!(cache.get("a"), None);
    }

    #[test]
    fn insert_if_absent_accepts_a_key_once() {
        let cache = TtlCache::new(Duration::from_secs(5));
        assert!(cache.insert_if_absent("a", 1));
        assert!(!cache.insert_if_absent("a", 2));
        assert_eq!(cache.get("a"), Some(1));
    }

    #[test]
    fn insert_evicts_expired_entries() {
        let cache = TtlCache::new(Duration::from_millis(40));
        cache.insert("old", 1);
        std::thread::sleep(Duration::from_millis(60));
        cache.insert("new", 2);
        assert_eq!(cache.entries.lock().unwrap().len(), 1);
        assert_eq!(cache.get("new"), Some(2));
    }
}
