//! MCP JSON-RPC protocol types — declarative, no hardcoding.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// JSON-RPC 2.0 request.
#[derive(Debug, Serialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: &'static str,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Deserialize)]
pub struct JsonRpcResponse {
    pub id: Option<u64>,
    pub result: Option<Value>,
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
}

/// MCP initialize params.
pub fn initialize_request(id: u64) -> JsonRpcRequest {
    JsonRpcRequest::new(id, "initialize", Some(serde_json::json!({
        "protocolVersion": "2024-11-05",
        "capabilities": {
            "tools": {},
            "resources": {}
        },
        "clientInfo": {
            "name": "harmonia",
            "version": "0.2.0"
        }
    })))
}

/// MCP tools/list request.
pub fn list_tools_request(id: u64) -> JsonRpcRequest {
    JsonRpcRequest::new(id, "tools/list", Some(serde_json::json!({})))
}

/// MCP tools/call request.
pub fn call_tool_request(id: u64, name: &str, arguments: &Value) -> JsonRpcRequest {
    JsonRpcRequest::new(id, "tools/call", Some(serde_json::json!({
        "name": name,
        "arguments": arguments
    })))
}

/// MCP initialized notification (sent after initialize response).
pub fn initialized_notification() -> String {
    serde_json::to_string(&serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    })).unwrap_or_default()
}

/// MCP tool definition from tools/list response.
#[derive(Debug, Clone, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(rename = "inputSchema", default)]
    pub input_schema: Value,
}
