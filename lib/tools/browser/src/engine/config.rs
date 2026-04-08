//! Browser engine state and s-expression config parsing.

use std::sync::{OnceLock, RwLock};

/// Browser engine state.
pub struct BrowserState {
    pub(crate) user_agent: String,
    pub(crate) timeout_ms: u64,
    pub(crate) max_response_bytes: usize,
    pub(crate) network_allowlist: Option<Vec<String>>,
    pub(crate) initialized: bool,
}

impl Default for BrowserState {
    fn default() -> Self {
        Self {
            user_agent: "harmonia-browser/1.0.0".to_string(),
            timeout_ms: 10_000,
            max_response_bytes: 2 * 1024 * 1024, // 2 MB
            network_allowlist: None,
            initialized: false,
        }
    }
}

static STATE: OnceLock<RwLock<BrowserState>> = OnceLock::new();

pub fn state() -> &'static RwLock<BrowserState> {
    STATE.get_or_init(|| RwLock::new(BrowserState::default()))
}

/// Initialize the browser engine from an s-expression config string.
///
/// Recognized keys (parsed loosely):
///   :timeout <ms>
///   :user-agent "<string>"
///   :max-response-bytes <n>
///   :allowlist ("<domain1>" "<domain2>" ...)
pub fn init(config: &str) -> Result<(), String> {
    let mut st = state().write().map_err(|e| format!("lock poisoned: {e}"))?;

    if let Some(val) = parse_sexp_int(config, ":timeout") {
        st.timeout_ms = val as u64;
    }

    if let Some(val) = parse_sexp_string(config, ":user-agent") {
        st.user_agent = val;
    }

    if let Some(val) = parse_sexp_int(config, ":max-response-bytes") {
        st.max_response_bytes = val as usize;
    }

    if let Some(vals) = parse_sexp_string_list(config, ":allowlist") {
        st.network_allowlist = Some(vals);
    }

    st.initialized = true;
    Ok(())
}

// ---- S-expression config parsing utilities ----

pub(crate) fn parse_sexp_int(sexp: &str, key: &str) -> Option<i64> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    let end = after
        .find(|c: char| !c.is_ascii_digit() && c != '-')
        .unwrap_or(after.len());
    after[..end].parse().ok()
}

pub(crate) fn parse_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

pub(crate) fn parse_sexp_string_list(sexp: &str, key: &str) -> Option<Vec<String>> {
    let pos = sexp.find(key)?;
    let after = sexp[pos + key.len()..].trim_start();
    if !after.starts_with('(') {
        return None;
    }
    let rest = &after[1..];
    let end = rest.find(')')?;
    let list_str = &rest[..end];
    let items: Vec<String> = list_str
        .split('"')
        .enumerate()
        .filter_map(|(i, s)| {
            if i % 2 == 1 && !s.is_empty() {
                Some(s.to_string())
            } else {
                None
            }
        })
        .collect();
    if items.is_empty() {
        None
    } else {
        Some(items)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_sexp_int_works() {
        assert_eq!(
            parse_sexp_int("(:timeout 5000 :foo 1)", ":timeout"),
            Some(5000)
        );
    }

    #[test]
    fn parse_sexp_string_works() {
        assert_eq!(
            parse_sexp_string(r#"(:user-agent "MyBot/1.0")"#, ":user-agent"),
            Some("MyBot/1.0".to_string())
        );
    }
}
