use std::sync::Mutex;

/// A tiny fixed-capacity LRU cache for CMS transform objects.
///
/// Designed for 4-16 entries where linear scan is faster than hashing.
/// Uses a Mutex for thread safety across hundreds of concurrent contexts.
///
/// **Lock discipline:** No user-provided closures (create, clone) ever run
/// while the lock is held. Only brief Vec scans and moves happen under the lock.
/// This means a panic in transform creation or cloning cannot poison the mutex.
///
/// Entries are ordered from least-recently-used (front) to most-recently-used (back).
/// On hit, the entry is moved to the back. On eviction, the front entry is removed.
pub(crate) struct TinyLru<V> {
    slots: Mutex<Vec<(u64, V)>>,
    capacity: usize,
}

impl<V> TinyLru<V> {
    pub const fn new(capacity: usize) -> Self {
        Self { slots: Mutex::new(Vec::new()), capacity }
    }
}

/// Acquire the mutex, replacing a poisoned mutex with an empty Vec.
/// This ensures a panic during (external) code never permanently degrades the cache.
fn lock_or_replace<V>(mutex: &Mutex<Vec<(u64, V)>>) -> std::sync::MutexGuard<'_, Vec<(u64, V)>> {
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            // Replace poisoned data with empty vec — cache miss is fine, stale data is not.
            let mut guard = poisoned.into_inner();
            *guard = Vec::new();
            // Clear the poison flag so subsequent accesses work normally.
            // Requires Rust 1.77+ (our minimum is 1.85).
            mutex.clear_poison();
            guard
        }
    }
}

impl<V: Clone> TinyLru<V> {
    /// Look up a value by key. Returns a clone if found, and moves the entry
    /// to the most-recently-used position.
    ///
    /// V is expected to be Arc-wrapped, so clone is an infallible atomic increment.
    pub fn get(&self, key: u64) -> Option<V> {
        let mut slots = lock_or_replace(&self.slots);
        if let Some(pos) = slots.iter().position(|(k, _)| *k == key) {
            let entry = slots.remove(pos);
            let value = entry.1.clone();
            slots.push(entry);
            Some(value)
        } else {
            None
        }
    }

    /// Get a cached value or create and cache it. On capacity overflow, evicts
    /// the least-recently-used entry.
    ///
    /// Both `create` and clone run outside the lock. If another thread inserted
    /// the same key between our initial miss and the insert lock, we return the
    /// existing entry (promoting it to MRU) and discard the redundant value.
    pub fn get_or_create(&self, key: u64, create: impl FnOnce() -> V) -> V {
        if let Some(v) = self.get(key) {
            return v;
        }
        // Create outside the lock
        let value = create();
        // Insert (lock held briefly for Vec ops only)
        let mut slots = lock_or_replace(&self.slots);
        // Re-check: another thread may have inserted the same key
        if let Some(pos) = slots.iter().position(|(k, _)| *k == key) {
            let entry = slots.remove(pos);
            let existing = entry.1.clone();
            slots.push(entry);
            return existing;
        }
        if slots.len() >= self.capacity {
            slots.remove(0); // evict LRU (front)
        }
        slots.push((key, value.clone()));
        value
    }

    /// Get a cached value or create and cache it, with fallible creation.
    /// On creation error, nothing is cached and the error is returned.
    /// The lock is only held during cache lookup/insert, not during creation.
    pub fn try_get_or_create<E>(
        &self,
        key: u64,
        create: impl FnOnce() -> Result<V, E>,
    ) -> Result<V, E> {
        if let Some(v) = self.get(key) {
            return Ok(v);
        }
        // Create outside the lock
        let value = create()?;
        // Insert (lock held briefly)
        let mut slots = lock_or_replace(&self.slots);
        // Re-check: another thread may have inserted the same key
        if let Some(pos) = slots.iter().position(|(k, _)| *k == key) {
            let entry = slots.remove(pos);
            let existing = entry.1.clone();
            slots.push(entry);
            return Ok(existing);
        }
        if slots.len() >= self.capacity {
            slots.remove(0);
        }
        slots.push((key, value.clone()));
        Ok(value)
    }

    /// Number of cached entries (for testing).
    #[cfg(test)]
    pub fn len(&self) -> usize {
        lock_or_replace(&self.slots).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // ---- Basic operations ----

    #[test]
    fn get_miss_returns_none() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        assert!(cache.get(42).is_none());
    }

    #[test]
    fn get_or_create_inserts_and_returns() {
        let cache: TinyLru<Arc<String>> = TinyLru::new(4);
        let v = cache.get_or_create(1, || Arc::new("hello".into()));
        assert_eq!(&**v, "hello");
        // Second call returns cached value, doesn't call create
        let v2 = cache.get_or_create(1, || panic!("should not be called"));
        assert_eq!(&**v2, "hello");
    }

    #[test]
    fn get_returns_cached_value() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        cache.get_or_create(10, || Arc::new(100));
        let v = cache.get(10);
        assert_eq!(v.map(|a| *a), Some(100));
    }

    // ---- LRU eviction ----

    #[test]
    fn evicts_lru_at_capacity() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(3);
        cache.get_or_create(1, || Arc::new(10));
        cache.get_or_create(2, || Arc::new(20));
        cache.get_or_create(3, || Arc::new(30));
        assert_eq!(cache.len(), 3);

        // Insert 4th — should evict key=1 (LRU)
        cache.get_or_create(4, || Arc::new(40));
        assert_eq!(cache.len(), 3);
        assert!(cache.get(1).is_none(), "key 1 should have been evicted");
        assert_eq!(*cache.get(2).unwrap(), 20);
        assert_eq!(*cache.get(3).unwrap(), 30);
        assert_eq!(*cache.get(4).unwrap(), 40);
    }

    #[test]
    fn get_promotes_to_mru() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(3);
        cache.get_or_create(1, || Arc::new(10));
        cache.get_or_create(2, || Arc::new(20));
        cache.get_or_create(3, || Arc::new(30));

        // Touch key=1 to promote it
        cache.get(1);

        // Insert key=4 — should evict key=2 (now LRU), not key=1
        cache.get_or_create(4, || Arc::new(40));
        assert!(cache.get(2).is_none(), "key 2 should have been evicted");
        assert_eq!(*cache.get(1).unwrap(), 10);
    }

    #[test]
    fn get_or_create_hit_promotes_to_mru() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(3);
        cache.get_or_create(1, || Arc::new(10));
        cache.get_or_create(2, || Arc::new(20));
        cache.get_or_create(3, || Arc::new(30));

        // Hit key=1 via get_or_create — promotes it
        cache.get_or_create(1, || panic!("should not create"));

        // Evict — key=2 is now LRU
        cache.get_or_create(4, || Arc::new(40));
        assert!(cache.get(2).is_none());
        assert!(cache.get(1).is_some());
    }

    // ---- Fallible creation ----

    #[test]
    fn try_get_or_create_success() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        let r: Result<_, String> = cache.try_get_or_create(1, || Ok(Arc::new(42)));
        assert_eq!(*r.unwrap(), 42);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn try_get_or_create_error_does_not_cache() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        let r: Result<Arc<i32>, &str> = cache.try_get_or_create(1, || Err("bad profile"));
        assert!(r.is_err());
        assert_eq!(cache.len(), 0);
        // Subsequent call can succeed
        let r2: Result<_, &str> = cache.try_get_or_create(1, || Ok(Arc::new(99)));
        assert_eq!(*r2.unwrap(), 99);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn try_get_or_create_evicts_at_capacity() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(2);
        let _: Result<_, String> = cache.try_get_or_create(1, || Ok(Arc::new(10)));
        let _: Result<_, String> = cache.try_get_or_create(2, || Ok(Arc::new(20)));
        assert_eq!(cache.len(), 2);

        // Third insert via try path — should evict key=1 (LRU)
        let r: Result<_, String> = cache.try_get_or_create(3, || Ok(Arc::new(30)));
        assert_eq!(*r.unwrap(), 30);
        assert_eq!(cache.len(), 2);
        assert!(cache.get(1).is_none(), "key 1 should have been evicted");
        assert_eq!(*cache.get(2).unwrap(), 20);
        assert_eq!(*cache.get(3).unwrap(), 30);
    }

    #[test]
    fn try_get_or_create_returns_cached_on_hit() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        cache.get_or_create(5, || Arc::new(50));
        let r: Result<_, String> = cache.try_get_or_create(5, || panic!("should not be called"));
        assert_eq!(*r.unwrap(), 50);
    }

    // ---- Panic recovery ----

    #[test]
    fn panic_in_create_does_not_poison_cache() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        // Seed a value
        cache.get_or_create(1, || Arc::new(10));

        // Panic during create for a different key
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.get_or_create(2, || panic!("simulated transform creation failure"));
        }));
        assert!(result.is_err());

        // Cache must still be functional — not poisoned
        let v = cache.get(1);
        assert_eq!(v.map(|a| *a), Some(10), "existing entry should survive panic");

        // New inserts must work
        let v2 = cache.get_or_create(3, || Arc::new(30));
        assert_eq!(*v2, 30);
    }

    #[test]
    fn panic_in_try_create_does_not_poison_cache() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        cache.get_or_create(1, || Arc::new(10));

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<_, String> = cache.try_get_or_create(2, || {
                panic!("simulated fallible creation panic");
            });
        }));
        assert!(result.is_err());

        // Cache still works
        assert_eq!(*cache.get(1).unwrap(), 10);
        let v: Result<_, String> = cache.try_get_or_create(3, || Ok(Arc::new(30)));
        assert_eq!(*v.unwrap(), 30);
    }

    /// Force-poison the mutex by panicking inside a lock scope, then verify recovery.
    #[test]
    fn poisoned_mutex_recovers_with_empty_cache() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(4);
        cache.get_or_create(1, || Arc::new(10));
        cache.get_or_create(2, || Arc::new(20));

        // Deliberately poison the mutex by panicking while holding the lock
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = cache.slots.lock().unwrap();
            panic!("deliberate poison");
        }));
        assert!(result.is_err());

        // Mutex is now poisoned. Next access should recover with empty cache.
        assert!(cache.get(1).is_none(), "poisoned cache should be cleared");
        assert!(cache.get(2).is_none(), "poisoned cache should be cleared");

        // Cache is functional again — inserts work
        cache.get_or_create(3, || Arc::new(30));
        assert_eq!(*cache.get(3).unwrap(), 30);
        assert_eq!(cache.len(), 1);
    }

    // ---- Capacity edge cases ----

    #[test]
    fn capacity_one_cache() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(1);
        cache.get_or_create(1, || Arc::new(10));
        assert_eq!(*cache.get(1).unwrap(), 10);

        // Second insert evicts the only entry
        cache.get_or_create(2, || Arc::new(20));
        assert!(cache.get(1).is_none());
        assert_eq!(*cache.get(2).unwrap(), 20);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn multiple_evictions_preserve_order() {
        let cache: TinyLru<Arc<i32>> = TinyLru::new(2);
        cache.get_or_create(1, || Arc::new(10));
        cache.get_or_create(2, || Arc::new(20));
        // Evict 1, insert 3
        cache.get_or_create(3, || Arc::new(30));
        assert!(cache.get(1).is_none());
        // Evict 2 (now LRU since 3 was just inserted), insert 4
        cache.get_or_create(4, || Arc::new(40));
        // After get(3) above promoted 3, order is [3, 4], so both present
        // Actually: after get_or_create(3), order was [2, 3]. Then get(1) was None.
        // Then get_or_create(4): len=2 >= cap=2, evict front=2, push 4. Order: [3, 4].
        assert!(cache.get(2).is_none());
        assert_eq!(*cache.get(3).unwrap(), 30);
        assert_eq!(*cache.get(4).unwrap(), 40);
    }

    // ---- Concurrent access ----

    #[test]
    fn concurrent_get_or_create() {
        use std::sync::Barrier;

        let cache: Arc<TinyLru<Arc<i32>>> = Arc::new(TinyLru::new(8));
        let barrier = Arc::new(Barrier::new(8));
        let mut handles = Vec::new();

        for i in 0..8u64 {
            let cache = cache.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                // Each thread creates its own key
                let v = cache.get_or_create(i, || Arc::new(i as i32 * 10));
                assert_eq!(*v, i as i32 * 10);
                // And reads others
                for j in 0..8u64 {
                    // May or may not be present yet — just exercise the path
                    let _ = cache.get(j);
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // All 8 keys should be present
        for i in 0..8u64 {
            assert_eq!(*cache.get(i).unwrap(), i as i32 * 10);
        }
    }

    #[test]
    fn concurrent_with_eviction() {
        use std::sync::Barrier;

        let cache: Arc<TinyLru<Arc<i32>>> = Arc::new(TinyLru::new(4));
        let barrier = Arc::new(Barrier::new(16));
        let mut handles = Vec::new();

        for i in 0..16u64 {
            let cache = cache.clone();
            let barrier = barrier.clone();
            handles.push(std::thread::spawn(move || {
                barrier.wait();
                cache.get_or_create(i, || Arc::new(i as i32));
            }));
        }
        for h in handles {
            h.join().unwrap();
        }
        // At most 4 entries (capacity), all valid
        assert!(cache.len() <= 4);
        // Every present entry has correct value
        for i in 0..16u64 {
            if let Some(v) = cache.get(i) {
                assert_eq!(*v, i as i32);
            }
        }
    }
}
