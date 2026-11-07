use regex::Regex;
use serde_json::Value;

pub struct DocPruner {
    nav_regex: Regex,
    excessive_newlines: Regex,
}

impl DocPruner {
    pub fn new() -> Self {
        Self {
            nav_regex: Regex::new(r"(?m)^\s*\[.*?\]\(.*?\)\s*(?:[>|\-\x{2022}]\s*\[.*?\]\(.*?\)\s*)+$").unwrap(),
            excessive_newlines: Regex::new(r"\n{3,}").unwrap(),
        }
    }

    pub fn prune_text(&self, raw: &str) -> String {
        // Remove navigation breadcrumbs
        let without_nav = self.nav_regex.replace_all(raw, "");

        // Normalize whitespace and trailing spaces
        let cleaned = self.excessive_newlines.replace_all(&without_nav, "\n\n");

        cleaned.trim().to_string()
    }

    pub fn prune_doc_json(&self, mut val: Value, max_results: usize) -> Value {
        match val {
            Value::Array(ref mut items) => {
                if items.len() > max_results {
                    items.truncate(max_results);
                }
                for item in items.iter_mut() {
                    self.clean_single_doc_item(item);
                }
                val
            }
            Value::Object(_) => {
                self.clean_single_doc_item(&mut val);
                val
            }
            other => other,
        }
    }

    fn clean_single_doc_item(&self, item: &mut Value) {
        if let Value::Object(ref mut map) = item {
            if let Some(Value::String(ref s)) = map.get("description") {
                let pruned = self.prune_text(s);
                map.insert("description".to_string(), Value::String(pruned));
            }
            if let Some(Value::String(ref s)) = map.get("snippet") {
                let pruned = self.prune_text(s);
                map.insert("snippet".to_string(), Value::String(pruned));
            }
        }
    }
}

impl Default for DocPruner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_doc_pruning() {
        let pruner = DocPruner::default();
        let sample = r#"
[LOVE Wiki](https://love2d.org) > [love.graphics](https://love2d.org/wiki/graphics) > [draw](https://love2d.org/wiki/draw)

Draws the object on screen.



```lua
love.graphics.draw(img, 10, 10)
```
"#;

        let pruned = pruner.prune_text(sample);
        assert!(!pruned.contains("[LOVE Wiki]"));
        assert!(pruned.contains("Draws the object on screen."));
        assert!(!pruned.contains("\n\n\n"));
    }
}

