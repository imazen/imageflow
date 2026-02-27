use std::sync::Mutex;

/// A tiny fixed-capacity LRU cache for CMS transform objects.
///
/// Designed for 4-16 entries where linear scan is faster than hashing.
/// Uses a Mutex since contention is near-zero (one lookup per frame decode)
/// and lock hold time is sub-microsecond for scans of this size.
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

impl<V: Clone> TinyLru<V> {
    /// Look up a value by key. Returns a clone if found, and moves the entry
    /// to the most-recently-used position.
    pub fn get(&self, key: u64) -> Option<V> {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = slots.iter().position(|(k, _)| *k == key) {
            // Move to back (most recently used)
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
    pub fn get_or_create(&self, key: u64, create: impl FnOnce() -> V) -> V {
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(pos) = slots.iter().position(|(k, _)| *k == key) {
            let entry = slots.remove(pos);
            let value = entry.1.clone();
            slots.push(entry);
            return value;
        }
        // Not found — create, evict if needed, insert
        let value = create();
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
        // Check cache first (lock held briefly)
        if let Some(v) = self.get(key) {
            return Ok(v);
        }
        // Create outside the lock
        let value = create()?;
        // Insert into cache (lock held briefly)
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        if slots.len() >= self.capacity {
            slots.remove(0);
        }
        slots.push((key, value.clone()));
        Ok(value)
    }
}
