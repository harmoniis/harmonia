//! MCP server — exposes Harmonia's primitives as MCP tools.
//!
//! Other agents (Claude Code, Codex, Cursor) can call these tools
//! via MCP protocol. Bidirectional A2A communication.
//!
//! Tool calls are dispatched through the runtime's IPC socket to the
//! actual component actors. No stubs — real operations.

use serde_json::{json, Value};

/// Generate MCP tool definitions from Harmonia's primitives.
pub fn tool_definitions() -> Value {
    json!([
        {
            "name": "harmonia_field",
            "description": "Get Harmonia's global context map — basin status, concepts, memory state",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "harmonia_recall",
            "description": "Search Harmonia's knowledge palace for information",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "Search query" },
                    "limit": { "type": "integer", "description": "Max results", "default": 5 }
                },
                "required": ["query"]
            }
        },
        {
            "name": "harmonia_store",
            "description": "Store information in Harmonia's memory palace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "Content to store" }
                },
                "required": ["content"]
            }
        },
        {
            "name": "harmonia_exec",
            "description": "Execute a shell command through Harmonia's workspace",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "command": { "type": "string", "description": "Shell command" }
                },
                "required": ["command"]
            }
        },
        {
            "name": "harmonia_status",
            "description": "Get Harmonia's runtime status — cycle, tier, model, fluency",
            "inputSchema": { "type": "object", "properties": {} }
        },
        {
            "name": "harmonia_basin",
            "description": "Get memory field attractor basin status",
            "inputSchema": { "type": "object", "properties": {} }
        }
    ])
}

/// Map MCP tool name + arguments to an IPC sexp command.
/// Returns the sexp string to dispatch through the runtime.
fn tool_to_ipc_sexp(tool_name: &str, arguments: &Value) -> Result<String, String> {
    match tool_name {
        "harmonia_field" => Ok(
            "(:component \"memory-field\" :op \"status\")".to_string()
        ),
        "harmonia_recall" => {
            let query = arguments.get("query").and_then(|v| v.as_str()).unwrap_or("");
            let limit = arguments.get("limit").and_then(|v| v.as_u64()).unwrap_or(5);
            Ok(format!(
                "(:component \"mempalace\" :op \"search\" :query \"{}\" :limit {})",
                harmonia_actor_protocol::sexp_escape(query), limit
            ))
        }
        "harmonia_store" => {
            let content = arguments.get("content").and_then(|v| v.as_str()).unwrap_or("");
            Ok(format!(
                "(:component \"mempalace\" :op \"file-drawer\" :content \"{}\" :room 0)",
                harmonia_actor_protocol::sexp_escape(content)
            ))
        }
        "harmonia_exec" => {
            let command = arguments.get("command").and_then(|v| v.as_str()).unwrap_or("");
            Ok(format!(
                "(:component \"workspace\" :op \"exec\" :cmd \"{}\")",
                harmonia_actor_protocol::sexp_escape(command)
            ))
        }
        "harmonia_status" => Ok(
            "(:component \"signalograd\" :op \"status\")".to_string()
        ),
        "harmonia_basin" => Ok(
            "(:component \"memory-field\" :op \"basin-status\")".to_string()
        ),
        _ => Err(format!("unknown tool: {}", tool_name)),
    }
}

/// Handle an incoming MCP tools/call request.
/// Generates IPC sexp command and returns it for the caller to dispatch.
pub fn handle_tool_call(tool_name: &str, arguments: &Value) -> Result<Value, String> {
    let sexp = tool_to_ipc_sexp(tool_name, arguments)?;
    // Return the sexp command — the MCP actor's dispatch method
    // will route this through the runtime's IPC to the actual component.
    Ok(json!({
        "content": [{"type": "text", "text": sexp}],
        "_ipc_sexp": sexp
    }))
}

/// Handle an incoming MCP JSON-RPC request (for server mode).
pub fn handle_jsonrpc(request: &str) -> String {
    let parsed: Result<Value, _> = serde_json::from_str(request);
    let req = match parsed {
        Ok(v) => v,
        Err(e) => return json!({"jsonrpc": "2.0", "error": {"code": -32700, "message": e.to_string()}}).to_string(),
    };

    let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");
    let id = req.get("id").cloned();
    let params = req.get("params").cloned().unwrap_or(json!({}));

    let result = match method {
        "initialize" => Ok(json!({
            "protocolVersion": "2024-11-05",
            "capabilities": { "tools": {} },
            "serverInfo": { "name": "harmonia", "version": "0.2.0" }
        })),
        "tools/list" => Ok(json!({ "tools": tool_definitions() })),
        "tools/call" => {
            let tool_name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
            let arguments = params.get("arguments").cloned().unwrap_or(json!({}));
            handle_tool_call(tool_name, &arguments)
        }
        "notifications/initialized" => return String::new(),
        _ => Err(format!("unknown method: {}", method)),
    };

    match result {
        Ok(r) => json!({"jsonrpc": "2.0", "id": id, "result": r}).to_string(),
        Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": e}}).to_string(),
    }
}
