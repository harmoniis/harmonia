//! 2-Tool MCP surface for the browser + controlled fetch.
//!
//! Exposes exactly two tools to the agent:
//! - `browser_search`: fetch URL + extract with a named macro
//! - `browser_execute`: multi-step browser plan (multiple fetch+extract)
//!
//! Plus a controlled fetch utility for safe API calls.

use crate::controlled_fetch::{self, ControlledFetchConfig};
use crate::engine;
use crate::macros::BrowserMacro;
use crate::sandbox::{sandboxed_exec, SandboxConfig};
use crate::security;
use serde_json::{json, Value};

/// Return the MCP tool definitions as JSON (for skill.md and agent discovery).
pub fn mcp_tool_definitions() -> Value {
    json!([
        {
            "name": "browser_search",
            "description": "Fetch a URL and extract specific data using a named macro. Returns structured JSON wrapped in security boundary.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "URL to fetch" },
                    "macro": {
                        "type": "string",
                        "enum": ["title", "text", "links", "headings", "tables", "forms", "meta", "audio", "markdown", "structured", "smart"],
                        "description": "Extraction macro: title, text, links, headings, tables, forms, meta, audio, markdown, structured, smart"
                    },
                    "arg": {
                        "type": "string",
                        "description": "Optional argument for the macro (e.g., query for smart, hint for structured)"
                    }
                },
                "required": ["url", "macro"]
            }
        },
        {
            "name": "browser_execute",
            "description": "Execute a multi-step browser plan: fetch URL, run multiple extractions, return combined results. All in one call.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string" },
                                "macro": { "type": "string" },
                                "arg": { "type": "string" }
                            }
                        }
                    }
                },
                "required": ["steps"]
            }
        }
    ])
}

/// Execute the `browser_search` tool: fetch a single URL and extract data.
pub fn browser_search(url: &str, macro_name: &str, arg: &str) -> String {
    let sandbox_cfg = SandboxConfig::default();

    let url_owned = url.to_string();
    let macro_name_owned = macro_name.to_string();
    let arg_owned = arg.to_string();

    let result = sandboxed_exec(&sandbox_cfg, move || {
        // 1. Resolve the macro
        let browser_macro = BrowserMacro::from_name(&macro_name_owned, &arg_owned)
            .ok_or_else(|| format!("unknown macro: {}", macro_name_owned))?;

        // 2. Fetch the URL
        let html = engine::fetch(&url_owned)?;

        // 3. Strip scripts/styles
        let cleaned = engine::strip_scripts_and_styles(&html);

        // 4. Execute the macro
        let data = browser_macro.execute(&cleaned);

        // 5. Wrap in security boundary
        let label = format!("browser_search:{}", browser_macro.name());
        Ok(security::wrap_secure(&data, &label))
    });

    match result {
        Ok(wrapped) => wrapped,
        Err(e) => security::wrap_secure(&json!({"error": e}), "browser_search:error"),
    }
}

/// Execute the `browser_execute` tool: multi-step browser plan.
pub fn browser_execute(steps_json: &str) -> String {
    let steps: Vec<Value> = match serde_json::from_str(steps_json) {
        Ok(v) => v,
        Err(e) => {
            return security::wrap_secure(
                &json!({"error": format!("failed to parse steps: {}", e)}),
                "browser_execute:error",
            );
        }
    };

    let sandbox_cfg = SandboxConfig::default();
    let mut results = Vec::new();

    for (i, step) in steps.iter().enumerate() {
        let url = step
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let macro_name = step
            .get("macro")
            .and_then(|v| v.as_str())
            .unwrap_or("text")
            .to_string();
        let arg = step
            .get("arg")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let step_result = sandboxed_exec(&sandbox_cfg, move || {
            let browser_macro = BrowserMacro::from_name(&macro_name, &arg)
                .ok_or_else(|| format!("unknown macro: {}", macro_name))?;

            let html = engine::fetch(&url)?;
            let cleaned = engine::strip_scripts_and_styles(&html);
            let data = browser_macro.execute(&cleaned);
            Ok(data)
        });

        match step_result {
            Ok(data) => {
                let label = format!("browser_execute:step_{}", i);
                results.push(json!({
                    "step": i,
                    "url": step.get("url").unwrap_or(&json!(null)),
                    "result": security::wrap_secure(&data, &label),
                }));
            }
            Err(e) => {
                results.push(json!({
                    "step": i,
                    "url": step.get("url").unwrap_or(&json!(null)),
                    "error": e,
                }));
            }
        }
    }

    security::wrap_secure(
        &json!({"steps": results, "total": steps.len()}),
        "browser_execute",
    )
}

/// Controlled fetch for agent API calls — blocks dangerous targets.
///
/// This is the AgentBrowser.fetch() equivalent. All JS code that needs
/// HTTP access goes through this function, which enforces domain allowlists
/// and blocks internal/metadata endpoints.
pub fn browser_controlled_fetch(url: &str, method: &str, body: Option<&str>) -> String {
    let config = ControlledFetchConfig::default();
    controlled_fetch::mcp_controlled_fetch(url, method, body, &config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_definitions_has_two_tools() {
        let defs = mcp_tool_definitions();
        let arr = defs.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "browser_search");
        assert_eq!(arr[1]["name"], "browser_execute");
    }

    #[test]
    fn mcp_tool_definitions_include_audio_macro() {
        let defs = mcp_tool_definitions();
        let search_tool = &defs[0];
        let macro_enum = search_tool["inputSchema"]["properties"]["macro"]["enum"]
            .as_array()
            .unwrap();
        let macro_names: Vec<&str> = macro_enum.iter().map(|v| v.as_str().unwrap()).collect();
        assert!(macro_names.contains(&"audio"));
    }

    #[test]
    fn browser_search_unknown_macro_returns_error() {
        let result = browser_search("https://example.com", "nonexistent", "");
        assert!(result.contains("unknown macro") || result.contains("error"));
    }

    #[test]
    fn controlled_fetch_blocks_localhost() {
        let result = browser_controlled_fetch("http://localhost:8080/", "GET", None);
        assert!(result.contains("BLOCKED"));
        assert!(result.contains("security-boundary"));
    }

    #[test]
    fn controlled_fetch_blocks_metadata() {
        let result = browser_controlled_fetch("http://169.254.169.254/latest/", "GET", None);
        assert!(result.contains("BLOCKED"));
    }
}
