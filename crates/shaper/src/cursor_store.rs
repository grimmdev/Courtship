use std::collections::HashMap; // Fast key-value hash map storing active cursor continuations
use std::sync::{Arc, Mutex}; // Thread-safe synchronization primitives
use std::time::{Duration, Instant}; // High-resolution monotonic timers for TTL expiration
use serde_json::Value; // Arbitrary dynamic JSON value type

/// Internal representation of a sliced array payload awaiting agent retrieval.
struct CachedSlice {
    /// Remaining unconsumed JSON array elements.
    items: Vec<Value>,
    /// Current read pointer offset within the array.
    offset: usize,
    /// Monotonic timestamp when the continuation was stored.
    created_at: Instant,
}

/// In-memory pagination store for sliced large array payloads.
/// Allows agents to inspect huge entity lists in compact chunks without context blowouts.
#[derive(Clone)]
pub struct CursorStore {
    /// Map from generated cursor token string to cached slice state.
    cache: Arc<Mutex<HashMap<String, CachedSlice>>>,
    /// Time-to-live duration before unconsumed slices are pruned.
    ttl: Duration,
}

impl CursorStore {
    pub fn new(ttl: Duration) -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            ttl,
        }
    }

    pub fn store_continuation(&self, remaining_items: Vec<Value>) -> String {
        let cursor_id = format!("cur_{:x}", rand_u64());
        let slice = CachedSlice {
            items: remaining_items,
            offset: 0,
            created_at: Instant::now(),
        };

        let mut lock = self.cache.lock().unwrap();
        // Prune stale items on insert
        let ttl = self.ttl;
        lock.retain(|_, v| v.created_at.elapsed() < ttl);
        lock.insert(cursor_id.clone(), slice);

        cursor_id
    }

    pub fn fetch_chunk(&self, cursor_id: &str, chunk_size: usize) -> Option<(Vec<Value>, Option<String>)> {
        let mut lock = self.cache.lock().unwrap();

        if let Some(entry) = lock.get_mut(cursor_id) {
            if entry.created_at.elapsed() >= self.ttl {
                lock.remove(cursor_id);
                return None;
            }

            let start = entry.offset;
            let end = (start + chunk_size).min(entry.items.len());
            let chunk = entry.items[start..end].to_vec();
            entry.offset = end;

            if entry.offset >= entry.items.len() {
                // Fully consumed, remove cursor
                lock.remove(cursor_id);
                Some((chunk, None))
            } else {
                Some((chunk, Some(cursor_id.to_string())))
            }
        } else {
            None
        }
    }
}

impl Default for CursorStore {
    fn default() -> Self {
        Self::new(Duration::from_secs(300)) // 5 minutes TTL
    }
}

fn rand_u64() -> u64 {
    use std::time::SystemTime;
    let now = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
    (now ^ 0x517cc1b727220a95) as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_pagination_lifecycle() {
        let store = CursorStore::default();
        let items: Vec<Value> = (0..120).map(|i| serde_json::json!({ "id": i })).collect();

        // Store 120 items
        let cursor_id = store.store_continuation(items);

        // Fetch chunk of 50
        let (chunk1, next_cur) = store.fetch_chunk(&cursor_id, 50).unwrap();
        assert_eq!(chunk1.len(), 50);
        assert_eq!(chunk1[0]["id"], 0);
        assert!(next_cur.is_some());

        // Fetch second chunk of 50
        let (chunk2, next_cur2) = store.fetch_chunk(&cursor_id, 50).unwrap();
        assert_eq!(chunk2.len(), 50);
        assert_eq!(chunk2[0]["id"], 50);
        assert!(next_cur2.is_some());

        // Fetch final remaining 20
        let (chunk3, next_cur3) = store.fetch_chunk(&cursor_id, 50).unwrap();
        assert_eq!(chunk3.len(), 20);
        assert_eq!(chunk3[0]["id"], 100);
        assert!(next_cur3.is_none()); // Fully drained

        // Subsequent fetch returns None
        assert!(store.fetch_chunk(&cursor_id, 50).is_none());
    }
}

