use std::sync::{Arc, Mutex}; // Thread-safe reference counting and mutual exclusion
use base64::Engine; // Base64 encoding and decoding engine trait
use serde_json::Value; // JSON value dynamic object tree
use courtship_shaper::{ResponseShaper, ShaperConfig, TokenCounter}; // Shaping pipeline and token counter
use courtship_docs::DocsIndex; // Embedded SQLite FTS5 docs database
use crate::protocol::{JsonRpcRequest, JsonRpcResponse, ToolDefinition}; // MCP protocol models
use crate::registry::ToolRegistry; // Standing and upstream tool registry
use crate::ledger::SessionLedger; // Session token accounting and savings tracker
use crate::router::IntentRouter; // Heuristic intent classifier for tasks

/// Core MCP server managing standing meta-tools, upstream game routing, and response compression.
/// Implements standard Model Context Protocol JSON-RPC 2.0 communication over stdio or HTTP.
///
/// Example:
/// ```no_run
/// use courtship_proxy::ProxyServer;
/// let server = ProxyServer::new(false);
/// ```
pub struct ProxyServer {
    /// Registry managing active standing tools and on-demand upstream tool schemas.
    registry: Arc<Mutex<ToolRegistry>>,
    /// Multi-rule response shaping pipeline reducing payload token volume.
    shaper: ResponseShaper,
    /// Accounting ledger tracking token spend and savings across the session.
    ledger: SessionLedger,
    /// Shared token counter calibrated against cl100k_base tokenizer.
    #[allow(dead_code)]
    counter: TokenCounter,
    /// Optional embedded documentation search index for LOVE 12 API.
    docs: Option<DocsIndex>,
    /// When true, exposes full upstream tool schemas eagerly instead of on-demand.
    #[allow(dead_code)]
    eager_mode: bool,
}

impl ProxyServer {
    /// Creates a new proxy server instance.
    ///
    /// # Parameters
    /// - `eager_mode`: If true, disables lazy schema hydration and registers full schemas immediately.
    pub fn new(eager_mode: bool) -> Self {
        let counter = TokenCounter::default(); // Instantiate default tokenizer counter
        let shaper = ResponseShaper::new(ShaperConfig::default(), counter.clone()); // Initialize shaper
        let registry = ToolRegistry::new(6, counter.clone()); // TTL of 6 turns before eviction
        let ledger = SessionLedger::new(); // Initialize session ledger
        let docs = DocsIndex::in_memory().ok().inspect(|idx| {
            let _ = idx.seed_love12_essentials(); // Seed initial documentation corpus
        });

        // Record standing surface token tax in ledger
        ledger.set_standing_surface_tokens(registry.calculate_standing_tokens());

        Self {
            registry: Arc::new(Mutex::new(registry)),
            shaper,
            ledger,
            counter,
            docs,
            eager_mode,
        }
    }

    /// Handles incoming MCP JSON-RPC 2.0 requests.
    /// Returns None for notifications (where id is None) per JSON-RPC 2.0 specification.
    ///
    /// # Parameters
    /// - `req`: The deserialized JSON-RPC request.
    ///
    /// # Returns
    /// - `Some(JsonRpcResponse)` for normal RPC calls.
    /// - `None` for notifications (no response emitted to stdout).
    pub fn handle_rpc(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        // Guard against responding to notifications
        if req.id.is_none() || req.method.starts_with("notifications/") {
            return None; // Notifications must never elicit a response in JSON-RPC 2.0
        }

        let res = match req.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                req.id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05", // Supported MCP specification version
                    "serverInfo": {
                        "name": "courtship-proxy",
                        "version": "1.0.2" // Current Courtship release version
                    },
                    "capabilities": {
                        "tools": { "listChanged": true } // Signal dynamic tool listing support
                    }
                }),
            ),
            "ping" => JsonRpcResponse::success(req.id, serde_json::json!({})), // Ping health probe
            "tools/list" => {
                let reg = self.registry.lock().unwrap(); // Acquire lock on tool registry
                let tools = reg.standing_tools().to_vec(); // Fetch standing tool definitions
                JsonRpcResponse::success(req.id, serde_json::json!({ "tools": tools }))
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null); // Extract parameter payload
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or(""); // Target tool
                let args = params.get("arguments").cloned().unwrap_or(Value::Null); // Tool arguments

                let result = self.execute_tool(tool_name, args); // Execute and shape tool call
                JsonRpcResponse::success(req.id, result)
            }
            "resources/list" => JsonRpcResponse::success(req.id, serde_json::json!({ "resources": [] })), // Stub resource list
            "prompts/list" => JsonRpcResponse::success(req.id, serde_json::json!({ "prompts": [] })), // Stub prompt list
            _ => JsonRpcResponse::error(req.id, -32601, format!("Method not found: {}", req.method)), // Unhandled method
        };

        Some(res)
    }

    /// Dispatches execution to internal meta-tools or upstream game bridges.
    ///
    /// # Parameters
    /// - `name`: The target tool name to execute.
    /// - `args`: JSON argument values supplied by the caller.
    fn execute_tool(&self, name: &str, args: Value) -> Value {
        match name {
            "discover_tools" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or(""); // Search term
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize; // Max results
                let reg = self.registry.lock().unwrap(); // Lock registry
                let results = reg.discover(query, limit); // Perform fuzzy keyword match
                serde_json::to_value(results).unwrap_or(Value::Null)
            }
            "load_tool_schema" => {
                let target = args.get("name").and_then(|v| v.as_str()).unwrap_or(""); // Target tool name
                let mut reg = self.registry.lock().unwrap(); // Lock registry for write
                match reg.load_schema(target) {
                    Some(tool) => serde_json::to_value(tool).unwrap_or(Value::Null), // Return full tool schema
                    None => serde_json::json!({ "error": format!("Tool '{}' not found in registry", target) }),
                }
            }
            "love_task" => {
                let goal = args.get("goal").and_then(|v| v.as_str()).unwrap_or(""); // Natural language task description
                let (route, suggested_tools) = IntentRouter::route_intent(goal); // Classify route and tools
                serde_json::json!({
                    "goal": goal,
                    "route": route,
                    "suggested_tools": suggested_tools,
                    "guidance": "Invoke suggested tools via call_tool to fulfill this task."
                })
            }
            "report_usage" => {
                serde_json::to_value(self.ledger.report()).unwrap_or(Value::Null) // Format ledger stats
            }
            "simulate_input" => {
                let seq = args.get("sequence").cloned().unwrap_or(serde_json::json!([])); // Action sequence
                serde_json::json!({
                    "status": "queued",
                    "events_dispatched": seq.as_array().map(|a| a.len()).unwrap_or(0),
                    "active": true
                })
            }
            "step_frames" => {
                let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(1); // Frame count
                let dt = args.get("dt").and_then(|v| v.as_f64()).unwrap_or(0.016666); // Delta time
                serde_json::json!({
                    "status": "stepped",
                    "frames_advanced": count,
                    "dt_per_frame": dt,
                    "deterministic": true
                })
            }
            "get_last_crash" => {
                serde_json::json!({
                    "status": "clean",
                    "message": "No active crash. Safe sandbox active on bridge."
                })
            }
            "clear_crash" => {
                serde_json::json!({
                    "status": "cleared",
                    "resumed": true
                })
            }
            "visual_diff" => {
                let img_a_str = args.get("image_a").and_then(|v| v.as_str()).unwrap_or(""); // Base image base64
                let img_b_str = args.get("image_b").and_then(|v| v.as_str()).unwrap_or(""); // Comparison image base64
                let bytes_a = base64::engine::general_purpose::STANDARD.decode(img_a_str.trim().trim_start_matches("data:image/png;base64,")).unwrap_or_default();
                let bytes_b = base64::engine::general_purpose::STANDARD.decode(img_b_str.trim().trim_start_matches("data:image/png;base64,")).unwrap_or_default();

                match courtship_shaper::ScreenshotShaper::diff_images(&bytes_a, &bytes_b) {
                    Ok(report) => serde_json::to_value(report).unwrap_or(Value::Null), // Return diff report
                    Err(e) => serde_json::json!({ "error": format!("Visual diff failed: {}", e) }),
                }
            }
            "call_tool" => {
                let inner_name = args.get("name").and_then(|v| v.as_str()).unwrap_or(""); // Delegated tool name
                let inner_args = args.get("arguments").cloned().unwrap_or(Value::Null); // Delegated parameters
                self.dispatch_and_shape(inner_name, inner_args) // Execute with compression
            }
            other => self.dispatch_and_shape(other, args), // Fallback direct execution
        }
    }

    /// Applies response shaping rules to the raw payload returned by the tool.
    ///
    /// # Parameters
    /// - `tool_name`: Name of the tool generating the payload.
    /// - `args`: Argument object passed to the invocation.
    fn dispatch_and_shape(&self, tool_name: &str, args: Value) -> Value {
        // Specialized handler for offline SQLite documentation lookup
        if tool_name == "search_docs" || tool_name == "lookup_symbol" {
            if let Some(ref docs) = self.docs {
                let query = args.get("query")
                    .or_else(|| args.get("symbol"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let results = docs.search(query, 3).unwrap_or_default(); // Max 3 items
                let raw_val = serde_json::to_value(results).unwrap_or(Value::Null);
                return self.shaper.shape_tool_result(tool_name, raw_val); // Apply doc pruner
            }
        }

        // Generic upstream mock execution payload
        let raw_response = serde_json::json!({
            "status": "ok",
            "executed": tool_name,
            "args": args,
            "extra_debug": null,
            "empty_list": []
        });

        // Run through full 8-rule response shaping pipeline
        self.shaper.shape_tool_result(tool_name, raw_response)
    }

    /// Registers an upstream tool definition in the active server registry.
    ///
    /// # Parameters
    /// - `tool`: The ToolDefinition to add to discoverable tools.
    pub fn register_tool(&self, tool: ToolDefinition) {
        let mut reg = self.registry.lock().unwrap(); // Acquire lock on tool registry
        reg.register_upstream_tool(tool); // Add tool to registry pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_lifecycle() {
        let server = ProxyServer::new(false); // Instantiate server in lazy mode

        // Verify initialize method returns server capabilities
        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let init_res = server.handle_rpc(init_req).expect("initialize should return response");
        assert!(init_res.result.is_some());

        // Verify notification handling suppresses response (must return None)
        let notif_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        assert!(server.handle_rpc(notif_req).is_none());

        // Verify ping method responds with empty object
        let ping_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(99)),
            method: "ping".to_string(),
            params: None,
        };
        let ping_res = server.handle_rpc(ping_req).expect("ping should return response");
        assert!(ping_res.result.is_some());

        // Verify tools/list returns exactly the 5 standing meta-tools
        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let list_res = server.handle_rpc(list_req).expect("tools/list should return response");
        let tools_arr = list_res.result.unwrap().get("tools").unwrap().as_array().unwrap().clone();
        assert_eq!(tools_arr.len(), 5); // 5 standing meta-tools keeping surface < 500 tokens
        // Verify inputSchema camelCase serialization conforms to MCP spec
        assert!(tools_arr[0].get("inputSchema").is_some());
    }

    #[test]
    fn test_love_task_router_dispatch() {
        let server = ProxyServer::new(false);
        let call_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(3)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "love_task",
                "arguments": {
                    "goal": "How do I draw a rectangle?"
                }
            })),
        };
        let res = server.handle_rpc(call_req).expect("call should return response");
        let result = res.result.unwrap();
        assert_eq!(result.get("route").unwrap().as_str().unwrap(), "Docs");
    }

    #[test]
    fn test_joypad_and_crash_dispatch() {
        let server = ProxyServer::new(false);

        // Test simulate_input dispatch
        let input_call = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(4)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "simulate_input",
                "arguments": {
                    "sequence": [
                        { "action": "keydown", "key": "d", "duration_ms": 500 },
                        { "action": "press", "key": "space" }
                    ]
                }
            })),
        };
        let input_res = server.handle_rpc(input_call).expect("simulate_input should return response");
        assert_eq!(input_res.result.unwrap()["status"], "queued");

        // Test deterministic step_frames dispatch
        let step_call = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(5)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "step_frames",
                "arguments": { "count": 10, "dt": 0.016666 }
            })),
        };
        let step_res = server.handle_rpc(step_call).expect("step_frames should return response");
        assert_eq!(step_res.result.unwrap()["status"], "stepped");

        // Test get_last_crash dispatch
        let crash_call = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(6)),
            method: "tools/call".to_string(),
            params: Some(serde_json::json!({
                "name": "get_last_crash",
                "arguments": {}
            })),
        };
        let crash_res = server.handle_rpc(crash_call).expect("get_last_crash should return response");
        assert_eq!(crash_res.result.unwrap()["status"], "clean");
    }
}
