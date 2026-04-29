//! McpState — actor-owned state for all MCP connections.
//!
//! Generic MCP client: connects to ANY MCP server (A2A peers, tool servers,
//! database servers, API gateways — anything speaking MCP protocol).
//! No hardcoded server types. The model decides what to connect to.

use std::collections::HashMap;

use crate::peer::{ConnectInfo, McpPeer};

/// Actor-owned MCP state. Owns all connected peers.
pub struct McpState {
    peers: HashMap<String, McpPeer>,
}

impl McpState {
    pub fn new() -> Self {
        Self {
            peers: HashMap::new(),
        }
    }

    /// Connect to any MCP server. Generic — works with any MCP-compatible process.
    /// If command is None, derives from server name via config-store.
    pub fn connect(&mut self, server: &str, command: Option<&str>) -> Result<ConnectInfo, String> {
        if self.peers.contains_key(server) {
            return Err(format!("already connected to {}", server));
        }

        let cmd = match command {
            Some(c) => c.to_string(),
            None => resolve_server_command(server)?,
        };

        let peer = McpPeer::connect(server, &cmd)?;
        let info = ConnectInfo {
            tool_count: peer.tools.len(),
        };
        self.peers.insert(server.to_string(), peer);
        Ok(info)
    }

    /// Call a tool on a connected peer.
    pub fn call_tool(
        &mut self,
        server: &str,
        tool: &str,
        arguments: &str,
    ) -> Result<String, String> {
        let peer = self
            .peers
            .get_mut(server)
            .ok_or_else(|| format!("not connected to {}", server))?;
        peer.call_tool(tool, arguments)
    }

    /// List tools on a connected peer.
    pub fn list_tools(&self, server: &str) -> Result<String, String> {
        let peer = self
            .peers
            .get(server)
            .ok_or_else(|| format!("not connected to {}", server))?;
        Ok(peer.tools_sexp())
    }

    /// List all connected peers.
    pub fn list_peers(&self) -> String {
        self.peers
            .iter()
            .map(|(name, peer)| {
                format!(
                    "(:name \"{}\" :command \"{}\" :tools {})",
                    harmonia_actor_protocol::sexp_escape(name),
                    harmonia_actor_protocol::sexp_escape(&peer.command),
                    peer.tools.len(),
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Disconnect from a peer.
    pub fn disconnect(&mut self, server: &str) {
        self.peers.remove(server);
    }
}

impl Default for McpState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve server name to launch command. Config-driven, no hardcoding.
/// Reads from config-store: mcp/<server>/command
fn resolve_server_command(server: &str) -> Result<String, String> {
    // Try config-store first
    if let Ok(Some(cmd)) = harmonia_config_store::get_config("mcp", "servers", server) {
        if !cmd.is_empty() {
            return Ok(cmd);
        }
    }

    // Well-known servers (declarative defaults, not hardcoded logic)
    match server {
        "claude-code" => Ok("claude mcp serve".to_string()),
        _ => Err(format!(
            "unknown MCP server '{}' — set config mcp/servers/{} to the launch command",
            server, server
        )),
    }
}
