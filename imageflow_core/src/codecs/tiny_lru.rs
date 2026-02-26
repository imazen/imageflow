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
        let mut slots = self.slots.lock().unwrap();
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
        let mut slots = self.slots.lock().unwrap();
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
}

impl<V> TinyLru<V> {
    /// Get a cached value or create and cache it, applying a function to the result.
    ///
    /// For non-Clone values (like lcms2 transforms): the `apply` callback receives
    /// a reference to the cached value while the lock is held.
    /// The `create` function is fallible. On creation error, nothing is cached.
    pub fn try_get_or_create_apply<R, E>(
        &self,
        key: u64,
        create: impl FnOnce() -> Result<V, E>,
        apply: impl FnOnce(&V) -> R,
    ) -> Result<R, E> {
        let mut slots = self.slots.lock().unwrap();
        if let Some(pos) = slots.iter().position(|(k, _)| *k == key) {
            // Move to back (most recently used)
            let entry = slots.remove(pos);
            let result = apply(&entry.1);
            slots.push(entry);
            return Ok(result);
        }
        // Not found — create, evict if needed, insert, apply
        let value = create()?;
        let result = apply(&value);
        if slots.len() >= self.capacity {
            slots.remove(0); // evict LRU (front)
        }
        slots.push((key, value));
        Ok(result)
    }
}
