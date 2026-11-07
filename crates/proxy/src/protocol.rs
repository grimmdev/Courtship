use serde::{Deserialize, Serialize}; // Serialization and deserialization framework
use serde_json::Value; // Dynamic JSON value representation

/// Represents an incoming JSON-RPC 2.0 request or notification from an MCP client.
/// Used by ProxyServer to parse stdio and HTTP messages.
///
/// Example:
/// ```json
/// { "jsonrpc": "2.0", "id": 1, "method": "tools/list" }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    /// Protocol specification version, strictly "2.0".
    pub jsonrpc: String,
    /// Unique request identifier. When None, this request is a notification.
    pub id: Option<Value>,
    /// Target MCP method to invoke (e.g. "initialize", "tools/call", "ping").
    pub method: String,
    /// Parameter object or array passed to the method.
    #[serde(default)]
    pub params: Option<Value>,
}

/// Outgoing JSON-RPC 2.0 response returned to an MCP client.
/// Contains either a success result payload or a standardized error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    /// Protocol specification version, strictly "2.0".
    pub jsonrpc: String,
    /// Matching request identifier from the corresponding JsonRpcRequest.
    pub id: Option<Value>,
    /// Successful result payload. Omitted when an error occurs.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    /// Error payload. Omitted when the request succeeds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

/// Standardized JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    /// Integer error code (-32601 for Method Not Found, -32603 for Internal Error).
    pub code: i64,
    /// Human-readable explanation of the error.
    pub message: String,
    /// Optional additional diagnostic data attached to the error.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    /// Constructs a successful JSON-RPC response with the given ID and result payload.
    ///
    /// # Parameters
    /// - `id`: The matching request identifier.
    /// - `result`: The JSON payload returned to the caller.
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(), // Set JSON-RPC version
            id, // Retain caller request ID
            result: Some(result), // Store successful outcome
            error: None, // Clear error state
        }
    }

    /// Constructs an error JSON-RPC response with a standardized code and message.
    ///
    /// # Parameters
    /// - `id`: The matching request identifier.
    /// - `code`: JSON-RPC error code integer.
    /// - `message`: Explanatory error text.
    pub fn error(id: Option<Value>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".to_string(), // Set JSON-RPC version
            id, // Retain caller request ID
            result: None, // Clear result state
            error: Some(JsonRpcError {
                code, // Assign error code
                message: message.into(), // Convert error message
                data: None, // Optional extra details
            }),
        }
    }
}

/// Full tool definition conforming to the official Model Context Protocol schema.
/// Serializes input_schema as camelCase inputSchema for MCP clients.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Unique identifier for the tool, e.g. "discover_tools" or "simulate_input".
    pub name: String,
    /// Markdown or plain text description explaining what the tool does.
    pub description: String,
    /// JSON Schema object describing valid arguments accepted by the tool.
    #[serde(rename = "inputSchema")]
    pub input_schema: Value,
}

/// Lightweight summary representation of a tool used in discovery results to save tokens.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSummary {
    /// Tool identifier name.
    pub name: String,
    /// Single sentence summary of tool purpose.
    pub one_line: String,
    /// Estimated token cost hint for calling this tool.
    pub cost_hint: String,
}
