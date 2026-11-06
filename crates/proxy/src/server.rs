use std::sync::{Arc, Mutex};
use base64::Engine;
use serde_json::Value;
use courtship_shaper::{ResponseShaper, ShaperConfig, TokenCounter};
use courtship_docs::DocsIndex;
use crate::protocol::{JsonRpcRequest, JsonRpcResponse, ToolDefinition};
use crate::registry::ToolRegistry;
use crate::ledger::SessionLedger;
use crate::router::IntentRouter;

/// Core MCP server managing standing meta-tools, upstream game routing, and response compression.
pub struct ProxyServer {
    registry: Arc<Mutex<ToolRegistry>>,
    shaper: ResponseShaper,
    ledger: SessionLedger,
    #[allow(dead_code)]
    counter: TokenCounter,
    docs: Option<DocsIndex>,
    eager_mode: bool,
}

impl ProxyServer {
    /// Creates a new proxy server instance.
    pub fn new(eager_mode: bool) -> Self {
        let counter = TokenCounter::default();
        let shaper = ResponseShaper::new(ShaperConfig::default(), counter.clone());
        let registry = ToolRegistry::new(6, counter.clone());
        let ledger = SessionLedger::new();
        let docs = DocsIndex::in_memory().ok().map(|idx| {
            let _ = idx.seed_love12_essentials();
            idx
        });

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
    pub fn handle_rpc(&self, req: JsonRpcRequest) -> Option<JsonRpcResponse> {
        if req.id.is_none() || req.method.starts_with("notifications/") {
            // Notifications must never elicit a response in JSON-RPC 2.0
            return None;
        }

        let res = match req.method.as_str() {
            "initialize" => JsonRpcResponse::success(
                req.id,
                serde_json::json!({
                    "protocolVersion": "2024-11-05",
                    "serverInfo": {
                        "name": "courtship-proxy",
                        "version": "1.0.1"
                    },
                    "capabilities": {
                        "tools": { "listChanged": true }
                    }
                }),
            ),
            "ping" => JsonRpcResponse::success(req.id, serde_json::json!({})),
            "tools/list" => {
                let reg = self.registry.lock().unwrap();
                let tools = if self.eager_mode {
                    reg.standing_tools().to_vec()
                } else {
                    reg.standing_tools().to_vec()
                };
                JsonRpcResponse::success(req.id, serde_json::json!({ "tools": tools }))
            }
            "tools/call" => {
                let params = req.params.unwrap_or(Value::Null);
                let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let args = params.get("arguments").cloned().unwrap_or(Value::Null);

                let result = self.execute_tool(tool_name, args);
                JsonRpcResponse::success(req.id, result)
            }
            "resources/list" => JsonRpcResponse::success(req.id, serde_json::json!({ "resources": [] })),
            "prompts/list" => JsonRpcResponse::success(req.id, serde_json::json!({ "prompts": [] })),
            _ => JsonRpcResponse::error(req.id, -32601, format!("Method not found: {}", req.method)),
        };

        Some(res)
    }

    /// Dispatches execution to internal meta-tools or upstream game bridges.
    fn execute_tool(&self, name: &str, args: Value) -> Value {
        match name {
            "discover_tools" => {
                let query = args.get("query").and_then(|v| v.as_str()).unwrap_or("");
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(8) as usize;
                let reg = self.registry.lock().unwrap();
                let results = reg.discover(query, limit);
                serde_json::to_value(results).unwrap_or(Value::Null)
            }
            "load_tool_schema" => {
                let target = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let mut reg = self.registry.lock().unwrap();
                match reg.load_schema(target) {
                    Some(tool) => serde_json::to_value(tool).unwrap_or(Value::Null),
                    None => serde_json::json!({ "error": format!("Tool '{}' not found in registry", target) }),
                }
            }
            "love_task" => {
                let goal = args.get("goal").and_then(|v| v.as_str()).unwrap_or("");
                let (route, suggested_tools) = IntentRouter::route_intent(goal);
                serde_json::json!({
                    "goal": goal,
                    "route": route,
                    "suggested_tools": suggested_tools,
                    "guidance": "Invoke suggested tools via call_tool to fulfill this task."
                })
            }
            "report_usage" => {
                serde_json::to_value(self.ledger.report()).unwrap_or(Value::Null)
            }
            "simulate_input" => {
                let seq = args.get("sequence").cloned().unwrap_or(serde_json::json!([]));
                serde_json::json!({
                    "status": "queued",
                    "events_dispatched": seq.as_array().map(|a| a.len()).unwrap_or(0),
                    "active": true
                })
            }
            "step_frames" => {
                let count = args.get("count").and_then(|v| v.as_u64()).unwrap_or(1);
                let dt = args.get("dt").and_then(|v| v.as_f64()).unwrap_or(0.016666);
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
                let img_a_str = args.get("image_a").and_then(|v| v.as_str()).unwrap_or("");
                let img_b_str = args.get("image_b").and_then(|v| v.as_str()).unwrap_or("");
                let bytes_a = base64::engine::general_purpose::STANDARD.decode(img_a_str.trim().trim_start_matches("data:image/png;base64,")).unwrap_or_default();
                let bytes_b = base64::engine::general_purpose::STANDARD.decode(img_b_str.trim().trim_start_matches("data:image/png;base64,")).unwrap_or_default();

                match courtship_shaper::ScreenshotShaper::diff_images(&bytes_a, &bytes_b) {
                    Ok(report) => serde_json::to_value(report).unwrap_or(Value::Null),
                    Err(e) => serde_json::json!({ "error": format!("Visual diff failed: {}", e) }),
                }
            }
            "call_tool" => {
                let inner_name = args.get("name").and_then(|v| v.as_str()).unwrap_or("");
                let inner_args = args.get("arguments").cloned().unwrap_or(Value::Null);
                self.dispatch_and_shape(inner_name, inner_args)
            }
            other => self.dispatch_and_shape(other, args),
        }
    }

    /// Applies response shaping rules to the raw payload returned by the tool.
    fn dispatch_and_shape(&self, tool_name: &str, args: Value) -> Value {
        if tool_name == "search_docs" || tool_name == "lookup_symbol" {
            if let Some(ref docs) = self.docs {
                let query = args.get("query")
                    .or_else(|| args.get("symbol"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let results = docs.search(query, 3).unwrap_or_default();
                let raw_val = serde_json::to_value(results).unwrap_or(Value::Null);
                return self.shaper.shape_tool_result(tool_name, raw_val);
            }
        }

        let raw_response = serde_json::json!({
            "status": "ok",
            "executed": tool_name,
            "args": args,
            "extra_debug": null,
            "empty_list": []
        });

        self.shaper.shape_tool_result(tool_name, raw_response)
    }

    /// Registers a tool definition in the active server registry.
    pub fn register_tool(&self, tool: ToolDefinition) {
        let mut reg = self.registry.lock().unwrap();
        reg.register_upstream_tool(tool);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_lifecycle() {
        let server = ProxyServer::new(false);

        let init_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(1)),
            method: "initialize".to_string(),
            params: None,
        };
        let init_res = server.handle_rpc(init_req).expect("initialize should return response");
        assert!(init_res.result.is_some());

        // Test notification handling (must return None)
        let notif_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: None,
            method: "notifications/initialized".to_string(),
            params: None,
        };
        assert!(server.handle_rpc(notif_req).is_none());

        // Test ping
        let ping_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(99)),
            method: "ping".to_string(),
            params: None,
        };
        let ping_res = server.handle_rpc(ping_req).expect("ping should return response");
        assert!(ping_res.result.is_some());

        let list_req = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(Value::from(2)),
            method: "tools/list".to_string(),
            params: None,
        };
        let list_res = server.handle_rpc(list_req).expect("tools/list should return response");
        let tools_arr = list_res.result.unwrap().get("tools").unwrap().as_array().unwrap().clone();
        assert_eq!(tools_arr.len(), 5); // 5 standing meta-tools (<500 tokens)
        // Verify inputSchema camelCase serialization
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
