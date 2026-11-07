use std::collections::HashMap; // Fast key-value hash map for upstream tool lookups
use serde_json::Value; // Arbitrary dynamic JSON value type
use courtship_shaper::TokenCounter; // Token accounting calibrated against cl100k_base
use crate::protocol::{ToolDefinition, ToolSummary}; // Protocol representations of tools

/// Manages active standing meta-tools, dynamic upstream tool discovery, and turn-based eviction.
/// Ensures the standing context tax remains strictly below 500 tokens.
pub struct ToolRegistry {
    /// 5 permanent standing meta-tools exposed to the AI client (~340 tokens).
    standing_tools: Vec<ToolDefinition>,
    /// Upstream tools available for on-demand hydration and execution.
    upstream_tools: HashMap<String, ToolDefinition>,
    /// Map of currently hydrated tool names to remaining TTL turns before eviction.
    loaded_tools: HashMap<String, usize>,
    /// Number of turns a tool schema remains active in context without usage.
    evict_after_turns: usize,
    /// Token counter for measuring schema tax.
    token_counter: TokenCounter,
}

impl ToolRegistry {
    /// Creates a new tool registry with configured turn-based eviction.
    pub fn new(evict_after_turns: usize, counter: TokenCounter) -> Self {
        let mut registry = Self {
            standing_tools: Vec::new(),
            upstream_tools: HashMap::new(),
            loaded_tools: HashMap::new(),
            evict_after_turns,
            token_counter: counter,
        };
        registry.init_standing_tools();
        registry.init_builtin_upstream_tools();
        registry
    }

    /// Initializes the standing meta-tool schema surface (kept strictly < 500 tokens).
    fn init_standing_tools(&mut self) {
        self.standing_tools = vec![
            ToolDefinition {
                name: "discover_tools".to_string(),
                description: "Search available tools by keyword or intent without loading full schemas into context.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "query": { "type": "string", "description": "Keyword or action (e.g. 'physics', 'screenshot', 'joypad', 'crash')" },
                        "limit": { "type": "integer", "default": 8 }
                    },
                    "required": ["query"]
                }),
            },
            ToolDefinition {
                name: "load_tool_schema".to_string(),
                description: "Load the full JSON schema for a specific tool into active context on-demand.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "The exact tool name" }
                    },
                    "required": ["name"]
                }),
            },
            ToolDefinition {
                name: "call_tool".to_string(),
                description: "Invoke any tool through the proxy with automatic response compression and shaping.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "The target tool name" },
                        "arguments": { "type": "object", "description": "Tool parameters" }
                    },
                    "required": ["name", "arguments"]
                }),
            },
            ToolDefinition {
                name: "love_task".to_string(),
                description: "Intelligent router for LOVE development: routes to docs, live inspection, or code execution.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "goal": { "type": "string", "description": "What you want to accomplish or query" },
                        "context": { "type": "string", "description": "Optional background or current game state info" }
                    },
                    "required": ["goal"]
                }),
            },
            ToolDefinition {
                name: "report_usage".to_string(),
                description: "Returns the current session token ledger and savings breakdown.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ];
    }

    /// Builtin engine capabilities kept daemon-side to avoid polluting prompt context.
    fn init_builtin_upstream_tools(&mut self) {
        let tools = vec![
            ToolDefinition {
                name: "simulate_input".to_string(),
                description: "Agent Joypad: Dispatches timed keyboard and button sequences to test gameplay mechanics.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "sequence": {
                            "type": "array",
                            "items": {
                                "type": "object",
                                "properties": {
                                    "action": { "type": "string", "enum": ["press", "keydown", "keyup"] },
                                    "key": { "type": "string", "description": "Key code (e.g. 'space', 'd', 'left')" },
                                    "duration_ms": { "type": "integer", "default": 100 }
                                },
                                "required": ["key"]
                            }
                        }
                    },
                    "required": ["sequence"]
                }),
            },
            ToolDefinition {
                name: "step_frames".to_string(),
                description: "Deterministic frame stepping: Advances game physics and logic by N fixed-timestep frames.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "count": { "type": "integer", "default": 1, "description": "Number of frames to advance" },
                        "dt": { "type": "number", "default": 0.016666, "description": "Fixed delta time per frame in seconds" }
                    }
                }),
            },
            ToolDefinition {
                name: "get_last_crash".to_string(),
                description: "Safe Crash Diagnostics: Retrieves file, line number, traceback, and locals from intercepted panics.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            ToolDefinition {
                name: "clear_crash".to_string(),
                description: "Resumes normal game execution after applying a code hot-patch.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
            ToolDefinition {
                name: "visual_diff".to_string(),
                description: "Daemon-side perceptual image comparison returning localized bounding boxes and match percentages.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "image_a": { "type": "string", "description": "Base64 encoded reference image" },
                        "image_b": { "type": "string", "description": "Base64 encoded candidate image" }
                    },
                    "required": ["image_a", "image_b"]
                }),
            },
            ToolDefinition {
                name: "get_objects".to_string(),
                description: "Inspect active game entities and their positions.".to_string(),
                input_schema: serde_json::json!({ "type": "object", "properties": {} }),
            },
            ToolDefinition {
                name: "get_screenshot".to_string(),
                description: "Captures a compressed, scaled framebuffer snapshot of the game.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "scale": { "type": "number", "default": 0.5 },
                        "region": {
                            "type": "object",
                            "properties": {
                                "x": { "type": "integer" },
                                "y": { "type": "integer" },
                                "width": { "type": "integer" },
                                "height": { "type": "integer" }
                            }
                        }
                    }
                }),
            },
            ToolDefinition {
                name: "run_lua".to_string(),
                description: "Execute arbitrary Lua code within the live game state.".to_string(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "code": { "type": "string", "description": "Lua source code snippet" }
                    },
                    "required": ["code"]
                }),
            },
        ];

        for tool in tools {
            self.upstream_tools.insert(tool.name.clone(), tool);
        }
    }

    /// Registers a tool definition provided by an external engine bridge.
    pub fn register_upstream_tool(&mut self, tool: ToolDefinition) {
        self.upstream_tools.insert(tool.name.clone(), tool);
    }

    /// Returns the standing meta-tool slice.
    pub fn standing_tools(&self) -> &[ToolDefinition] {
        &self.standing_tools
    }

    /// Queries the registry for matching tools based on a text query.
    pub fn discover(&self, query: &str, limit: usize) -> Vec<ToolSummary> {
        let q = query.to_lowercase();
        self.upstream_tools
            .values()
            .filter(|t| t.name.to_lowercase().contains(&q) || t.description.to_lowercase().contains(&q))
            .take(limit)
            .map(|t| ToolSummary {
                name: t.name.clone(),
                one_line: t.description.lines().next().unwrap_or("").to_string(),
                cost_hint: "shaped (~50-150 tok)".to_string(),
            })
            .collect()
    }

    /// Loads the full JSON schema for a target tool, setting its active turn TTL.
    pub fn load_schema(&mut self, name: &str) -> Option<&ToolDefinition> {
        if let Some(tool) = self.upstream_tools.get(name) {
            self.loaded_tools.insert(name.to_string(), self.evict_after_turns);
            Some(tool)
        } else {
            None
        }
    }

    /// Decrements TTL turns for loaded schemas, evicting expired tools.
    pub fn tick_turn(&mut self) {
        let mut expired = Vec::new();
        for (name, turns) in self.loaded_tools.iter_mut() {
            if *turns > 1 {
                *turns -= 1;
            } else {
                expired.push(name.clone());
            }
        }
        for name in expired {
            self.loaded_tools.remove(&name);
        }
    }

    /// Calculates token count of the standing meta-tool surface.
    pub fn calculate_standing_tokens(&self) -> usize {
        let val = serde_json::to_value(&self.standing_tools).unwrap_or(Value::Null);
        self.token_counter.count_json_tokens(&val)
    }

    /// Calculates token count if all upstream schemas were loaded eagerly.
    pub fn calculate_all_upstream_tokens(&self) -> usize {
        let all: Vec<&ToolDefinition> = self.upstream_tools.values().collect();
        let val = serde_json::to_value(&all).unwrap_or(Value::Null);
        self.token_counter.count_json_tokens(&val)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standing_surface_tokens() {
        let counter = TokenCounter::default();
        let registry = ToolRegistry::new(6, counter);
        let standing_tokens = registry.calculate_standing_tokens();

        println!("Standing surface tokens measured: {}", standing_tokens);
        // Architectural target: standing surface stays well below 500 tokens
        assert!(standing_tokens < 500, "Standing tokens was {standing_tokens}, exceeded 500 limit!");
    }

    #[test]
    fn test_tool_discovery_and_eviction() {
        let counter = TokenCounter::default();
        let mut registry = ToolRegistry::new(2, counter);

        let found = registry.discover("joypad", 5);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "simulate_input");

        assert!(registry.load_schema("simulate_input").is_some());
        assert_eq!(registry.loaded_tools.len(), 1);

        registry.tick_turn();
        assert_eq!(registry.loaded_tools.len(), 1);

        registry.tick_turn(); // Evicts after 2 inactive turns
        assert_eq!(registry.loaded_tools.len(), 0);
    }
}
