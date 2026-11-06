use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use crate::tokenizer::TokenCounter;
use crate::cursor_store::CursorStore;
use crate::doc_pruner::DocPruner;
use crate::screenshot::{ScreenshotOptions, ScreenshotShaper};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaperConfig {
    pub strip_empty: bool,
    pub max_depth: usize,
    pub max_array_len: usize,
    pub max_tokens: usize,
    pub lean_object_fields: Vec<String>,
}

impl Default for ShaperConfig {
    fn default() -> Self {
        Self {
            strip_empty: true,
            max_depth: 3,
            max_array_len: 50,
            max_tokens: 1500,
            lean_object_fields: vec![
                "id".to_string(),
                "type".to_string(),
                "name".to_string(),
                "x".to_string(),
                "y".to_string(),
            ],
        }
    }
}

pub struct ResponseShaper {
    config: ShaperConfig,
    counter: TokenCounter,
    cursor_store: CursorStore,
    doc_pruner: DocPruner,
}

impl ResponseShaper {
    pub fn new(config: ShaperConfig, counter: TokenCounter) -> Self {
        Self {
            config,
            counter,
            cursor_store: CursorStore::default(),
            doc_pruner: DocPruner::default(),
        }
    }

    pub fn cursor_store(&self) -> &CursorStore {
        &self.cursor_store
    }

    pub fn shape_tool_result(&self, tool_name: &str, mut value: Value) -> Value {
        // 1. Screenshot shaping
        if tool_name == "get_screenshot" {
            if let Some(b64) = value.get("image").or_else(|| value.get("image_base64")).and_then(|v| v.as_str()) {
                if let Ok(shaped) = ScreenshotShaper::shape_base64(b64, &ScreenshotOptions::default()) {
                    return serde_json::to_value(shaped).unwrap_or(value);
                }
            }
        }

        // 2. Doc pruning
        if tool_name == "search_docs" || tool_name == "lookup_symbol" {
            value = self.doc_pruner.prune_doc_json(value, 3);
        }

        // 3. Tool-specific field projection (e.g., get_objects)
        if tool_name == "get_objects" {
            value = self.project_objects(value);
        }

        // 4. Depth cap & array truncation with cursor continuation
        let shaped = self.apply_depth_and_caps(value, 0);

        // 5. Null / empty stripping
        let cleaned = if self.config.strip_empty {
            self.strip_empty_values(shaped).unwrap_or(Value::Object(Map::new()))
        } else {
            shaped
        };

        // 6. Token budget ceiling
        self.enforce_token_budget(cleaned)
    }

    fn project_objects(&self, val: Value) -> Value {
        match val {
            Value::Array(items) => {
                let projected = items
                    .into_iter()
                    .map(|item| self.project_single_object(item))
                    .collect();
                Value::Array(projected)
            }
            Value::Object(map) => {
                if let Some(Value::Array(items)) = map.get("objects").cloned() {
                    let mut new_map = map.clone();
                    let projected = items
                        .into_iter()
                        .map(|item| self.project_single_object(item))
                        .collect();
                    new_map.insert("objects".to_string(), Value::Array(projected));
                    Value::Object(new_map)
                } else {
                    self.project_single_object(Value::Object(map))
                }
            }
            other => other,
        }
    }

    fn project_single_object(&self, val: Value) -> Value {
        if let Value::Object(map) = val {
            let mut out = Map::new();
            for field in &self.config.lean_object_fields {
                if let Some(v) = map.get(field) {
                    out.insert(field.clone(), v.clone());
                }
            }
            Value::Object(out)
        } else {
            val
        }
    }

    fn apply_depth_and_caps(&self, val: Value, current_depth: usize) -> Value {
        match val {
            Value::Object(map) => {
                if current_depth >= self.config.max_depth {
                    return Value::String(format!("<table: {} keys>", map.len()));
                }
                let mut out = Map::new();
                for (k, v) in map {
                    out.insert(k, self.apply_depth_and_caps(v, current_depth + 1));
                }
                Value::Object(out)
            }
            Value::Array(mut list) => {
                if current_depth >= self.config.max_depth {
                    return Value::String(format!("<array: {} items>", list.len()));
                }
                let original_len = list.len();
                if original_len > self.config.max_array_len {
                    let remaining: Vec<Value> = list.drain(self.config.max_array_len..).collect();
                    let cursor_id = self.cursor_store.store_continuation(remaining);

                    let mut capped_list: Vec<Value> = list
                        .into_iter()
                        .map(|item| self.apply_depth_and_caps(item, current_depth + 1))
                        .collect();
                    let cursor_hint = serde_json::json!({
                        "_cursor": cursor_id,
                        "_remaining": original_len - self.config.max_array_len
                    });
                    capped_list.push(cursor_hint);
                    Value::Array(capped_list)
                } else {
                    Value::Array(
                        list.into_iter()
                            .map(|item| self.apply_depth_and_caps(item, current_depth + 1))
                            .collect(),
                    )
                }
            }
            primitive => primitive,
        }
    }

    fn strip_empty_values(&self, val: Value) -> Option<Value> {
        match val {
            Value::Null => None,
            Value::String(s) if s.trim().is_empty() => None,
            Value::Array(items) => {
                let stripped: Vec<Value> = items
                    .into_iter()
                    .filter_map(|item| self.strip_empty_values(item))
                    .collect();
                if stripped.is_empty() {
                    None
                } else {
                    Some(Value::Array(stripped))
                }
            }
            Value::Object(map) => {
                let stripped: Map<String, Value> = map
                    .into_iter()
                    .filter_map(|(k, v)| self.strip_empty_values(v).map(|clean_v| (k, clean_v)))
                    .collect();
                if stripped.is_empty() {
                    None
                } else {
                    Some(Value::Object(stripped))
                }
            }
            other => Some(other),
        }
    }

    fn enforce_token_budget(&self, mut val: Value) -> Value {
        let tokens = self.counter.count_json_tokens(&val);
        if tokens <= self.config.max_tokens {
            return val;
        }

        if let Value::Object(ref mut map) = val {
            map.insert("_truncated".to_string(), Value::Bool(true));
            map.insert(
                "_token_budget_exceeded".to_string(),
                serde_json::json!({
                    "measured_tokens": tokens,
                    "max_allowed": self.config.max_tokens
                }),
            );
        } else if let Value::Array(ref mut list) = val {
            while self.counter.count_json_tokens(&Value::Array(list.clone())) > self.config.max_tokens
                && list.len() > 1
            {
                list.pop();
            }
            list.push(serde_json::json!({
                "_truncated": true,
                "_message": "Token budget exceeded, subsequent items truncated"
            }));
        }
        val
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_null_and_empty_stripping() {
        let shaper = ResponseShaper::new(ShaperConfig::default(), TokenCounter::default());
        let raw = serde_json::json!({
            "name": "player",
            "empty_str": "",
            "null_val": null,
            "empty_obj": {},
            "empty_arr": [],
            "valid_num": 42
        });

        let shaped = shaper.shape_tool_result("custom_tool", raw);
        assert_eq!(shaped, serde_json::json!({
            "name": "player",
            "valid_num": 42
        }));
    }

    #[test]
    fn test_cursor_array_continuation() {
        let shaper = ResponseShaper::new(
            ShaperConfig {
                max_array_len: 2,
                ..Default::default()
            },
            TokenCounter::default(),
        );

        let large_array = serde_json::json!([
            {"id": 1},
            {"id": 2},
            {"id": 3},
            {"id": 4}
        ]);

        let shaped = shaper.shape_tool_result("custom_tool", large_array);
        let arr = shaped.as_array().unwrap();
        assert_eq!(arr.len(), 3); // 2 items + 1 cursor hint
        let cursor_hint = &arr[2];
        let cursor_id = cursor_hint.get("_cursor").unwrap().as_str().unwrap();

        // Validate retrieval from cursor store
        let (next_chunk, next_id) = shaper.cursor_store().fetch_chunk(cursor_id, 2).unwrap();
        assert_eq!(next_chunk.len(), 2);
        assert_eq!(next_chunk[0]["id"], 3);
        assert_eq!(next_chunk[1]["id"], 4);
        assert!(next_id.is_none());
    }
}
