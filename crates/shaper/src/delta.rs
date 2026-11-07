use std::collections::HashMap; // Fast key-value hash map storing state snapshots
use std::sync::{Arc, Mutex}; // Thread-safe synchronization primitives
use serde_json::{Map, Value}; // Dynamic JSON object and value representations

/// Thread-safe delta tracking engine for continuous game state streams.
/// Suppresses identical ticks to zero tokens and emits minimal JSON diffs on mutation.
#[derive(Debug, Clone, Default)]
pub struct DeltaTracker {
    /// In-memory map from stream identifier to previous full JSON state snapshot.
    snapshots: Arc<Mutex<HashMap<String, Value>>>,
}

impl DeltaTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Computes delta between the last recorded snapshot and the new value.
    /// If there is no previous snapshot, records it and returns the full value with `_initial: true`.
    /// If there is a previous snapshot, returns only mutated/added keys with `_delta: true`.
    pub fn compute_delta(&self, stream_id: &str, current: Value) -> Option<Value> {
        let mut map = self.snapshots.lock().unwrap();

        match map.get(stream_id) {
            None => {
                map.insert(stream_id.to_string(), current.clone());
                let mut out = current;
                if let Value::Object(ref mut o) = out {
                    o.insert("_initial_snapshot".to_string(), Value::Bool(true));
                }
                Some(out)
            }
            Some(previous) => {
                let diff = Self::diff_values(previous, &current);
                if diff.is_null() {
                    None // Zero change, skip emission to save 100% of tokens!
                } else {
                    map.insert(stream_id.to_string(), current);
                    Some(diff)
                }
            }
        }
    }

    /// Clears the snapshot for a given stream
    pub fn clear(&self, stream_id: &str) {
        let mut map = self.snapshots.lock().unwrap();
        map.remove(stream_id);
    }

    /// Internal recursive JSON diff
    fn diff_values(old_val: &Value, new_val: &Value) -> Value {
        if old_val == new_val {
            return Value::Null;
        }

        match (old_val, new_val) {
            (Value::Object(old_map), Value::Object(new_map)) => {
                let mut changes = Map::new();

                // Check added or changed keys
                for (k, new_v) in new_map {
                    match old_map.get(k) {
                        Some(old_v) => {
                            let sub_diff = Self::diff_values(old_v, new_v);
                            if !sub_diff.is_null() {
                                changes.insert(k.clone(), sub_diff);
                            }
                        }
                        None => {
                            changes.insert(k.clone(), new_v.clone());
                        }
                    }
                }

                // Check deleted keys
                for k in old_map.keys() {
                    if !new_map.contains_key(k) {
                        changes.insert(format!("_removed_{}", k), Value::Bool(true));
                    }
                }

                if changes.is_empty() {
                    Value::Null
                } else {
                    let mut delta_obj = Map::new();
                    delta_obj.insert("_delta".to_string(), Value::Bool(true));
                    delta_obj.insert("changes".to_string(), Value::Object(changes));
                    Value::Object(delta_obj)
                }
            }
            // For primitives or arrays, return new value
            (_, new_v) => new_v.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_delta_tracker_lifecycle() {
        let tracker = DeltaTracker::new();
        let state1 = serde_json::json!({
            "player": { "x": 100, "y": 200, "hp": 100 },
            "score": 50
        });

        // Initial snapshot
        let res1 = tracker.compute_delta("game_state", state1).unwrap();
        assert!(res1.get("_initial_snapshot").is_some());

        // Unchanged state -> None (0 tokens emitted!)
        let state2 = serde_json::json!({
            "player": { "x": 100, "y": 200, "hp": 100 },
            "score": 50
        });
        let res2 = tracker.compute_delta("game_state", state2);
        assert!(res2.is_none());

        // Mutated player x coordinate only
        let state3 = serde_json::json!({
            "player": { "x": 115, "y": 200, "hp": 100 },
            "score": 50
        });
        let res3 = tracker.compute_delta("game_state", state3).unwrap();
        assert!(res3.get("_delta").unwrap().as_bool().unwrap());

        let changes = res3.get("changes").unwrap();
        assert!(changes.get("player").is_some());
        // score was not modified, so it shouldn't be in the delta
        assert!(changes.get("score").is_none());
    }
}

