//! Harmonia MCP — Model Context Protocol A2A client/server.
//!
//! Bidirectional: Harmonia calls tools on peers, peers call tools on Harmonia.
//! All state actor-owned. Service pattern: Cmd/Delta/Ok.
//!
//! Architecture:
//! - McpState: owns connected peers, their tool catalogs, pending requests
//! - connect(): launch peer process via stdio, perform MCP initialize handshake
//! - call_tool(): send tools/call JSON-RPC, await response
//! - list_tools(): query peer's tool catalog
//! - serve(): handle incoming JSON-RPC requests (Harmonia as MCP server)

mod peer;
mod protocol;
pub mod server;
mod state;

pub use state::McpState;

use harmonia_actor_protocol::extract_sexp_string;

/// Dispatch MCP commands via actor-owned state. Pure functional dispatch.
pub fn dispatch(state: &mut McpState, sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "connect" => {
            let server = extract_sexp_string(sexp, ":server").unwrap_or_default();
            let command = extract_sexp_string(sexp, ":command");
            match state.connect(&server, command.as_deref()) {
                Ok(info) => format!("(:ok :server \"{}\" :tools {})",
                    harmonia_actor_protocol::sexp_escape(&server), info.tool_count),
                Err(e) => format!("(:error \"mcp connect: {}\")",
                    harmonia_actor_protocol::sexp_escape(&e)),
            }
        }
        "call-tool" => {
            let server = extract_sexp_string(sexp, ":server").unwrap_or_default();
            let tool = extract_sexp_string(sexp, ":tool").unwrap_or_default();
            let arguments = extract_sexp_string(sexp, ":arguments").unwrap_or_default();
            match state.call_tool(&server, &tool, &arguments) {
                Ok(result) => format!("(:ok :result \"{}\")",
                    harmonia_actor_protocol::sexp_escape(&result)),
                Err(e) => format!("(:error \"mcp call: {}\")",
                    harmonia_actor_protocol::sexp_escape(&e)),
            }
        }
        "list-tools" => {
            let server = extract_sexp_string(sexp, ":server").unwrap_or_default();
            match state.list_tools(&server) {
                Ok(tools) => format!("(:ok :tools ({}))", tools),
                Err(e) => format!("(:error \"mcp list-tools: {}\")",
                    harmonia_actor_protocol::sexp_escape(&e)),
            }
        }
        "list-peers" => {
            let peers = state.list_peers();
            format!("(:ok :peers ({}))", peers)
        }
        "disconnect" => {
            let server = extract_sexp_string(sexp, ":server").unwrap_or_default();
            state.disconnect(&server);
            "(:ok)".to_string()
        }
        "serve" => {
            // Handle an incoming MCP JSON-RPC request (Harmonia as MCP server)
            let request = extract_sexp_string(sexp, ":request").unwrap_or_default();
            let response = server::handle_jsonrpc(&request);
            format!("(:ok :response \"{}\")",
                harmonia_actor_protocol::sexp_escape(&response))
        }
        "tool-definitions" => {
            let defs = server::tool_definitions();
            format!("(:ok :tools \"{}\")",
                harmonia_actor_protocol::sexp_escape(&defs.to_string()))
        }
        "healthcheck" => "(:ok :status \"mcp-ready\")".to_string(),
        _ => format!("(:error \"unknown mcp op: {}\")",
            harmonia_actor_protocol::sexp_escape(&op)),
    }
}
