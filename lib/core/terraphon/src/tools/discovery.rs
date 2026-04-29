//! MCP and agent discovery tools — scan the system for MCP-compatible tools.
//!
//! Registered as terraphon lodes. The agent can datamine these to discover
//! what tools are available for A2A communication.

use crate::catalog::LodeCatalog;
use crate::tools::{CpuCost, NetCost, Domain, Precondition};
use crate::tools::declare_system_tool;
use super::system::exec_capture;

/// Register MCP discovery lodes. Called from register_universal_tools.
pub fn register_discovery_tools(catalog: &mut LodeCatalog) {
    // Discover Claude Code installations
    catalog.register(declare_system_tool!(
        id: "discover-claude-code", platform: crate::platform::Platform::Any, domain: Domain::System,
        cost: (200, CpuCost::Low, NetCost::None),
        preconditions: [Precondition::BinaryExists("claude".into())],
        mine: |_args| {
            let version = exec_capture("claude", &["--version"]).unwrap_or_default();
            let mcp_check = exec_capture("claude", &["mcp", "list"]).unwrap_or_else(|_| "mcp-unavailable".into());
            Ok(format!(
                "(:tool \"claude-code\" :version \"{}\" :mcp-available {} :mcp-servers \"{}\")",
                version.trim(),
                if mcp_check.contains("mcp-unavailable") { "nil" } else { "t" },
                mcp_check.trim().replace('"', "\\\""),
            ))
        }
    ));

    // Discover Codex installations
    catalog.register(declare_system_tool!(
        id: "discover-codex", platform: crate::platform::Platform::Any, domain: Domain::System,
        cost: (200, CpuCost::Low, NetCost::None),
        preconditions: [Precondition::BinaryExists("codex".into())],
        mine: |_args| {
            let version = exec_capture("codex", &["--version"]).unwrap_or_default();
            Ok(format!("(:tool \"codex\" :version \"{}\" :available t)", version.trim()))
        }
    ));

    // Discover Cursor installations
    catalog.register(declare_system_tool!(
        id: "discover-cursor", platform: crate::platform::Platform::Any, domain: Domain::System,
        cost: (200, CpuCost::Low, NetCost::None),
        preconditions: [Precondition::BinaryExists("cursor".into())],
        mine: |_args| {
            let version = exec_capture("cursor", &["--version"]).unwrap_or_default();
            Ok(format!("(:tool \"cursor\" :version \"{}\" :available t)", version.trim()))
        }
    ));

    // Generic MCP server discovery — scan common paths
    catalog.register(declare_system_tool!(
        id: "discover-mcp-servers", platform: crate::platform::Platform::Any, domain: Domain::System,
        cost: (500, CpuCost::Low, NetCost::None),
        preconditions: [],
        mine: |_args| {
            let mut found = Vec::new();

            // Check for Claude Code MCP configuration
            let home = std::env::var("HOME").unwrap_or_default();
            let claude_config = format!("{}/.claude/settings.json", home);
            if std::path::Path::new(&claude_config).exists() {
                found.push("(:server \"claude-code\" :config-path \"~/.claude/settings.json\")".to_string());
            }

            // Check for npx-based MCP servers
            for server in &["@anthropic-ai/mcp-server-filesystem", "@anthropic-ai/mcp-server-github"] {
                let check = exec_capture("npx", &["--yes", "--package", server, "--", "--help"]);
                if check.is_ok() {
                    found.push(format!("(:server \"{}\" :type :npx)", server));
                }
            }

            if found.is_empty() {
                Ok("(:discovered 0)".to_string())
            } else {
                Ok(format!("(:discovered {} :servers ({}))", found.len(), found.join(" ")))
            }
        }
    ));
}
