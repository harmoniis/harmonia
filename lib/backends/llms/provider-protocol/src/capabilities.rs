//! Model Capability Protocol — provider-agnostic native tool discovery.
//!
//! Models declare their native tools (reasoning, web-search, x-search) in
//! `config/model-policy.sexp` under `:native-tools`. At boot, Lisp seeds
//! these into config-store under the `model-capabilities` scope.
//!
//! Backends call `model_capabilities(model_id)` to get a structured
//! `ModelCapabilities` — no hardcoded model checks needed.

/// Native capabilities a model may declare.
#[derive(Debug, Clone, Default)]
pub struct ModelCapabilities {
    pub reasoning: Option<ReasoningConfig>,
    pub web_search: Option<SearchConfig>,
    pub x_search: Option<SearchConfig>,
}

/// Reasoning / chain-of-thought activation parameters.
#[derive(Debug, Clone)]
pub struct ReasoningConfig {
    pub enabled: bool,
    pub effort: String,
    pub exclude: bool,
}

/// Native search (web or X/Twitter) activation parameters.
#[derive(Debug, Clone)]
pub struct SearchConfig {
    pub plugin_id: String,
    pub engine: String,
    pub search_context_size: String,
}

/// Look up native capabilities for a model.
///
/// Tries config-store first (seeded by Lisp at boot), falls back to
/// hardcoded defaults for known models so the system works even before
/// config-store is initialized.
pub fn model_capabilities(model: &str) -> ModelCapabilities {
    if let Some(caps) = read_from_config_store(model) {
        return caps;
    }
    hardcoded_fallback(model)
}

/// Check if a model has a specific native tool.
pub fn model_has_native_tool(model: &str, tool: &str) -> bool {
    let caps = model_capabilities(model);
    match tool {
        "reasoning" => caps.reasoning.is_some(),
        "web-search" => caps.web_search.is_some(),
        "x-search" => caps.x_search.is_some(),
        _ => false,
    }
}

// ── Config-store reader ──────────────────────────────────────────────

fn read_from_config_store(model: &str) -> Option<ModelCapabilities> {
    let raw = harmonia_config_store::get_config("provider-protocol", "model-capabilities", model)
        .ok()
        .flatten()?;
    parse_sexp_capabilities(&raw)
}

/// Minimal sexp plist parser for `:native-tools` format.
///
/// Input: `(:REASONING (:ENABLED T :EFFORT "high" :EXCLUDE T) :WEB-SEARCH ...)`
/// Extracts keyword-value pairs from a flat plist with nested sub-plists.
fn parse_sexp_capabilities(sexp: &str) -> Option<ModelCapabilities> {
    let mut caps = ModelCapabilities::default();
    let trimmed = sexp.trim();
    if !trimmed.starts_with('(') || !trimmed.ends_with(')') {
        return None;
    }
    let inner = &trimmed[1..trimmed.len() - 1];

    // Extract top-level sections
    if let Some(reasoning_sexp) = extract_plist_value(inner, ":REASONING") {
        caps.reasoning = parse_reasoning_config(&reasoning_sexp);
    }
    if let Some(web_sexp) = extract_plist_value(inner, ":WEB-SEARCH") {
        caps.web_search = parse_search_config(&web_sexp);
    }
    if let Some(x_sexp) = extract_plist_value(inner, ":X-SEARCH") {
        caps.x_search = parse_search_config(&x_sexp);
    }

    // Only return if we found at least one capability
    if caps.reasoning.is_some() || caps.web_search.is_some() || caps.x_search.is_some() {
        Some(caps)
    } else {
        None
    }
}

/// Extract the value following a keyword in a plist string.
/// Returns the balanced parenthesized expression or atom after the keyword.
fn extract_plist_value(plist: &str, key: &str) -> Option<String> {
    let upper = plist.to_ascii_uppercase();
    let pos = upper.find(key)?;
    let after_key = &plist[pos + key.len()..];
    let trimmed = after_key.trim_start();

    if trimmed.starts_with('(') {
        // Read balanced parens
        let mut depth = 0i32;
        let mut end = 0;
        for (i, ch) in trimmed.char_indices() {
            match ch {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        if end > 0 {
            Some(trimmed[..end].to_string())
        } else {
            None
        }
    } else {
        // Read atom (until whitespace or next keyword)
        let end = trimmed
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(trimmed.len());
        if end > 0 {
            Some(trimmed[..end].to_string())
        } else {
            None
        }
    }
}

/// Extract a quoted string value from a sub-plist.
fn extract_string_value(plist: &str, key: &str) -> Option<String> {
    let upper = plist.to_ascii_uppercase();
    let pos = upper.find(key)?;
    let after = plist[pos + key.len()..].trim_start();
    if after.starts_with('"') {
        let end = after[1..].find('"')?;
        Some(after[1..1 + end].to_string())
    } else {
        // Unquoted atom
        let end = after
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(after.len());
        Some(after[..end].to_string())
    }
}

/// Check if a boolean keyword is T (true) in a sub-plist.
fn extract_bool_value(plist: &str, key: &str) -> bool {
    let upper = plist.to_ascii_uppercase();
    if let Some(pos) = upper.find(key) {
        let after = upper[pos + key.len()..].trim_start();
        after.starts_with('T') && !after[1..].starts_with(|c: char| c.is_alphanumeric())
    } else {
        false
    }
}

fn parse_reasoning_config(sexp: &str) -> Option<ReasoningConfig> {
    let enabled = extract_bool_value(sexp, ":ENABLED");
    if !enabled {
        return None;
    }
    Some(ReasoningConfig {
        enabled: true,
        effort: extract_string_value(sexp, ":EFFORT").unwrap_or_else(|| "high".to_string()),
        exclude: extract_bool_value(sexp, ":EXCLUDE"),
    })
}

fn parse_search_config(sexp: &str) -> Option<SearchConfig> {
    Some(SearchConfig {
        plugin_id: extract_string_value(sexp, ":PLUGIN-ID").unwrap_or_else(|| "web".to_string()),
        engine: extract_string_value(sexp, ":ENGINE").unwrap_or_else(|| "native".to_string()),
        search_context_size: extract_string_value(sexp, ":SEARCH-CONTEXT-SIZE")
            .unwrap_or_else(|| "high".to_string()),
    })
}

// ── Hardcoded fallback ───────────────────────────────────────────────

fn hardcoded_fallback(model: &str) -> ModelCapabilities {
    let lower = model.to_ascii_lowercase();
    if lower.contains("grok-4.1-fast") {
        ModelCapabilities {
            reasoning: Some(ReasoningConfig {
                enabled: true,
                effort: "high".to_string(),
                exclude: true,
            }),
            web_search: Some(SearchConfig {
                plugin_id: "web".to_string(),
                engine: "native".to_string(),
                search_context_size: "high".to_string(),
            }),
            x_search: Some(SearchConfig {
                plugin_id: "web".to_string(),
                engine: "native".to_string(),
                search_context_size: "high".to_string(),
            }),
        }
    } else if lower.contains("grok") {
        // Other Grok models get reasoning but not native search
        ModelCapabilities {
            reasoning: Some(ReasoningConfig {
                enabled: true,
                effort: "high".to_string(),
                exclude: true,
            }),
            ..ModelCapabilities::default()
        }
    } else {
        ModelCapabilities::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_grok_native_tools() {
        let sexp = r#"(:REASONING (:ENABLED T :EFFORT "high" :EXCLUDE T) :WEB-SEARCH (:PLUGIN-ID "web" :ENGINE "native" :SEARCH-CONTEXT-SIZE "high") :X-SEARCH (:PLUGIN-ID "web" :ENGINE "native" :SEARCH-CONTEXT-SIZE "high"))"#;
        let caps = parse_sexp_capabilities(sexp).expect("should parse");
        let r = caps.reasoning.expect("reasoning");
        assert!(r.enabled);
        assert_eq!(r.effort, "high");
        assert!(r.exclude);
        let ws = caps.web_search.expect("web-search");
        assert_eq!(ws.plugin_id, "web");
        assert_eq!(ws.engine, "native");
        assert_eq!(ws.search_context_size, "high");
        let xs = caps.x_search.expect("x-search");
        assert_eq!(xs.plugin_id, "web");
    }

    #[test]
    fn fallback_grok_fast() {
        let caps = hardcoded_fallback("x-ai/grok-4.1-fast");
        assert!(caps.reasoning.is_some());
        assert!(caps.web_search.is_some());
        assert!(caps.x_search.is_some());
    }

    #[test]
    fn fallback_grok_other() {
        let caps = hardcoded_fallback("x-ai/grok-4");
        assert!(caps.reasoning.is_some());
        assert!(caps.web_search.is_none());
    }

    #[test]
    fn fallback_non_grok() {
        let caps = hardcoded_fallback("anthropic/claude-sonnet-4.6");
        assert!(caps.reasoning.is_none());
        assert!(caps.web_search.is_none());
    }

    #[test]
    fn has_native_tool_check() {
        assert!(model_has_native_tool("x-ai/grok-4.1-fast", "web-search"));
        assert!(!model_has_native_tool(
            "anthropic/claude-sonnet-4.6",
            "web-search"
        ));
    }

    #[test]
    fn empty_sexp_returns_none() {
        assert!(parse_sexp_capabilities("()").is_none());
        assert!(parse_sexp_capabilities("").is_none());
    }
}
