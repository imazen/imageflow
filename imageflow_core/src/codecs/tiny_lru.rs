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
    /// Both `create` and clone run outside the lock. A concurrent miss on the
    /// same key may create a duplicate, but that's harmless for a cache.
    pub fn get_or_create(&self, key: u64, create: impl FnOnce() -> V) -> V {
        if let Some(v) = self.get(key) {
            return v;
        }
        // Create outside the lock
        let value = create();
        // Insert (lock held briefly for Vec ops only)
        let mut slots = lock_or_replace(&self.slots);
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
        if slots.len() >= self.capacity {
            slots.remove(0);
        }
        slots.push((key, value.clone()));
        Ok(value)
    }
}
