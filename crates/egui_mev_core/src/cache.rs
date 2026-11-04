//! A cache bounded by both entry count and payload bytes.

use std::{borrow::Borrow, hash::Hash};

use lru::LruCache;

/// A least-recently-used cache that holds to two limits at once: how many
/// entries it keeps and how many payload bytes those add up to.
pub struct WeightedLru<K: Hash + Eq, V> {
    entries: LruCache<K, (V, usize)>,
    capacity: usize,
    byte_limit: usize,
    bytes: usize,
}

impl<K: Hash + Eq, V> WeightedLru<K, V> {
    /// An empty cache bounded by entry count only.
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self::with_byte_limit(capacity, usize::MAX)
    }

    /// An empty cache bounded by entry count and payload bytes.
    #[must_use]
    pub fn with_byte_limit(capacity: usize, byte_limit: usize) -> Self {
        Self {
            entries: LruCache::unbounded(),
            capacity,
            byte_limit,
            bytes: 0,
        }
    }

    /// Fetch a value, promoting it to the most recently used.
    pub fn get<Q>(&mut self, key: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.get(key).map(|(value, _)| value)
    }

    /// Insert a value with no weight.
    pub fn insert(&mut self, key: K, value: V) {
        self.insert_weighted(key, value, 0);
    }

    /// Insert a payload of `bytes`, evicting the oldest entries until both
    /// limits hold. A payload over the byte limit is dropped, along with any
    /// older value under the same key.
    pub fn insert_weighted(&mut self, key: K, value: V, bytes: usize) {
        self.remove(&key);
        if self.capacity == 0 || bytes > self.byte_limit {
            return;
        }
        while self.entries.len() >= self.capacity
            || self.bytes.saturating_add(bytes) > self.byte_limit
        {
            let Some((_, (_, evicted))) = self.entries.pop_lru() else {
                break;
            };
            self.bytes -= evicted;
        }
        self.bytes += bytes;
        self.entries.put(key, (value, bytes));
    }

    /// Payload bytes retained, not counting allocator and driver overhead.
    #[must_use]
    pub const fn bytes(&self) -> usize {
        self.bytes
    }

    /// Whether an entry is present, without promoting it.
    pub fn contains<Q>(&self, key: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        self.entries.contains(key)
    }

    /// Remove and return a value, if present.
    pub fn remove<Q>(&mut self, key: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq + ?Sized,
    {
        let (value, bytes) = self.entries.pop(key)?;
        self.bytes -= bytes;
        Some(value)
    }

    /// Remove every entry.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    /// Number of entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache holds nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::WeightedLru;

    #[test]
    fn byte_budget_evicts_incrementally_and_rejects_oversized_values() {
        let mut cache = WeightedLru::<String, i32>::with_byte_limit(5, 10);
        cache.insert_weighted("a".into(), 1, 4);
        cache.insert_weighted("b".into(), 2, 4);
        let _ = cache.get("a");
        cache.insert_weighted("c".into(), 3, 4);
        assert!(!cache.contains("b"));
        assert_eq!(cache.bytes(), 8);
        cache.insert_weighted("huge".into(), 4, 11);
        assert!(!cache.contains("huge"));
        assert_eq!(cache.bytes(), 8);
        cache.insert_weighted("a".into(), 5, 1);
        assert_eq!(cache.bytes(), 5);
        cache.clear();
        assert_eq!(cache.bytes(), 0);
    }

    #[test]
    fn zero_capacity_keeps_nothing() {
        let mut zero = WeightedLru::<String, i32>::with_byte_limit(0, 0);
        zero.insert("x".into(), 1);
        assert!(zero.is_empty());
    }

    #[test]
    fn evicts_least_recently_used_when_full() {
        let mut cache = WeightedLru::<&str, i32>::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        cache.insert("c", 3);
        assert_eq!(cache.len(), 2);
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn get_promotes_recently_used() {
        let mut cache = WeightedLru::<&str, i32>::new(2);
        cache.insert("a", 1);
        cache.insert("b", 2);
        // Touch "a" so "b" becomes the oldest.
        assert!(cache.get("a").is_some());
        cache.insert("c", 3);
        assert!(cache.get("a").is_some());
        assert!(cache.get("b").is_none());
    }

    #[test]
    fn insert_existing_key_updates_without_growth() {
        let mut cache = WeightedLru::<&str, i32>::new(2);
        cache.insert_weighted("a", 1, 3);
        cache.insert_weighted("a", 2, 5);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.bytes(), 5);
        assert_eq!(cache.get("a"), Some(&2));
    }

    #[test]
    fn remove_returns_the_value_and_frees_its_bytes() {
        let mut cache = WeightedLru::<&str, i32>::new(4);
        cache.insert_weighted("a", 1, 3);
        assert_eq!(cache.remove("a"), Some(1));
        assert_eq!(cache.remove("a"), None);
        assert_eq!(cache.bytes(), 0);
    }
}
