//! MCP peer connection — manages a single connected MCP server process.
//!
//! Each peer is a child process communicating via stdio JSON-RPC.
//! Actor-owned: the McpState owns all peers, no singletons.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};

use crate::protocol::{self, JsonRpcResponse, McpTool};

/// A connected MCP peer (child process with stdio transport).
pub struct McpPeer {
    pub name: String,
    pub command: String,
    pub tools: Vec<McpTool>,
    child: Child,
    next_id: u64,
}

/// Connection info returned after successful handshake.
pub struct ConnectInfo {
    pub tool_count: usize,
}

impl McpPeer {
    /// Connect to an MCP server by launching the command and performing handshake.
    pub fn connect(name: &str, command: &str) -> Result<Self, String> {
        // Launch the MCP server process with stdio transport
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err("empty command".to_string());
        }

        let mut child = Command::new(parts[0])
            .args(&parts[1..])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("spawn {}: {}", command, e))?;

        let mut peer = McpPeer {
            name: name.to_string(),
            command: command.to_string(),
            tools: Vec::new(),
            child,
            next_id: 1,
        };

        // MCP handshake: initialize → initialized notification → tools/list
        let init_id = peer.next_id();
        peer.send_and_receive(&protocol::initialize_request(init_id))?;

        // Send initialized notification
        let notif = protocol::initialized_notification();
        peer.send_raw(&notif)?;

        // Discover tools
        let tools_id = peer.next_id();
        let tools_resp = peer.send_and_receive(&protocol::list_tools_request(tools_id))?;
        if let Some(result) = tools_resp.result {
            if let Some(tools_array) = result.get("tools").and_then(|t| t.as_array()) {
                for tool_val in tools_array {
                    if let Ok(tool) = serde_json::from_value::<McpTool>(tool_val.clone()) {
                        peer.tools.push(tool);
                    }
                }
            }
        }

        Ok(peer)
    }

    fn next_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Send a JSON-RPC request and read the response.
    pub fn send_and_receive(
        &mut self,
        request: &protocol::JsonRpcRequest,
    ) -> Result<JsonRpcResponse, String> {
        let json = serde_json::to_string(request).map_err(|e| e.to_string())?;
        self.send_raw(&json)?;
        self.read_response()
    }

    fn send_raw(&mut self, json: &str) -> Result<(), String> {
        let stdin = self.child.stdin.as_mut().ok_or("no stdin")?;
        writeln!(stdin, "{}", json).map_err(|e| format!("write: {}", e))?;
        stdin.flush().map_err(|e| format!("flush: {}", e))?;
        Ok(())
    }

    fn read_response(&mut self) -> Result<JsonRpcResponse, String> {
        let stdout = self.child.stdout.as_mut().ok_or("no stdout")?;
        let mut reader = BufReader::new(stdout);
        let mut line = String::new();
        reader
            .read_line(&mut line)
            .map_err(|e| format!("read: {}", e))?;
        serde_json::from_str(&line).map_err(|e| format!("parse: {}", e))
    }

    /// Call a tool on this peer.
    pub fn call_tool(&mut self, tool_name: &str, arguments: &str) -> Result<String, String> {
        let args: serde_json::Value =
            serde_json::from_str(arguments).unwrap_or(serde_json::json!({}));
        let request = protocol::call_tool_request(self.next_id(), tool_name, &args);
        let response = self.send_and_receive(&request)?;

        if let Some(error) = response.error {
            return Err(format!("{}: {}", error.code, error.message));
        }

        match response.result {
            Some(result) => Ok(serde_json::to_string(&result).unwrap_or_default()),
            None => Ok("null".to_string()),
        }
    }

    /// List tools as sexp.
    pub fn tools_sexp(&self) -> String {
        self.tools
            .iter()
            .map(|t| {
                format!(
                    "(:name \"{}\" :description \"{}\")",
                    harmonia_actor_protocol::sexp_escape(&t.name),
                    harmonia_actor_protocol::sexp_escape(&t.description),
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }
}

impl Drop for McpPeer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
