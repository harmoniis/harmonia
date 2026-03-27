//! S-expression escaping and extraction — the shared sexp toolkit.
//!
//! Used by: runtime dispatch, IPC handler, all 9 frontends, benchmarks.
//! ONE set of extractors for the entire codebase.

/// Escape a string for embedding in sexp double-quoted values.
/// Only backslash and double-quote need escaping for CL's reader.
pub fn escape(input: &str) -> String {
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Extract a quoted or unquoted string value after the given key.
/// Handles escaped quotes (\"), backslashes (\\), and control chars.
pub fn extract_string(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = &sexp[idx + key.len()..];
    let after = after.trim_start();
    if !after.starts_with('"') {
        let val: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ')')
            .collect();
        return if val.is_empty() { None } else { Some(val) };
    }
    let bytes = after[1..].as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            return Some(result);
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'"' => result.push('"'),
                b'\\' => result.push('\\'),
                b'n' => result.push('\n'),
                b'r' => result.push('\r'),
                b't' => result.push('\t'),
                other => {
                    result.push('\\');
                    result.push(other as char);
                }
            }
            i += 2;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    None
}

/// Extract a u64 value after the given key.
pub fn extract_u64(sexp: &str, key: &str) -> Option<u64> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    let num: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
    if num.is_empty() { None } else { num.parse().ok() }
}

/// Extract a u64 with default.
pub fn extract_u64_or(sexp: &str, key: &str, default: u64) -> u64 {
    extract_u64(sexp, key).unwrap_or(default)
}

/// Extract a f64 value after the given key.
pub fn extract_f64(sexp: &str, key: &str) -> Option<f64> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    let num: String = after
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
        .collect();
    if num.is_empty() { None } else { num.parse().ok() }
}

/// Extract a list of quoted strings: (:key ("a" "b" "c")).
pub fn extract_string_list(sexp: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    let Some(pos) = sexp.find(key) else { return items };
    let rest = &sexp[pos + key.len()..];
    let Some(open) = rest.find('(') else { return items };
    let inner = &rest[open + 1..];
    let Some(close) = inner.find(')') else { return items };
    let content = &inner[..close];
    let mut in_quote = false;
    let mut current = String::new();
    let mut escaped = false;
    for ch in content.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
        } else if ch == '\\' && in_quote {
            escaped = true;
        } else if ch == '"' && !in_quote {
            in_quote = true;
            current.clear();
        } else if ch == '"' && in_quote {
            in_quote = false;
            if !current.is_empty() {
                items.push(current.clone());
            }
        } else if in_quote {
            current.push(ch);
        }
    }
    items
}

/// Extract boolean: t → true, nil → false.
pub fn extract_bool(sexp: &str, key: &str) -> Option<bool> {
    match extract_string(sexp, key)?.as_str() {
        "t" => Some(true),
        "nil" => Some(false),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escape_quotes_and_backslashes() {
        assert_eq!(escape(r#"hello "world""#), r#"hello \"world\""#);
        assert_eq!(escape(r"path\to\file"), r"path\\to\\file");
    }

    #[test]
    fn extract_string_quoted() {
        let sexp = r#"(:component "vault" :op "set-secret" :value "sk-123")"#;
        assert_eq!(extract_string(sexp, ":component"), Some("vault".into()));
        assert_eq!(extract_string(sexp, ":value"), Some("sk-123".into()));
    }

    #[test]
    fn extract_string_with_escapes() {
        let sexp = r#"(:text "hello \"world\"")"#;
        assert_eq!(extract_string(sexp, ":text"), Some(r#"hello "world""#.into()));
    }

    #[test]
    fn extract_string_unquoted() {
        assert_eq!(extract_string("(:kind gateway)", ":kind"), Some("gateway".into()));
    }

    #[test]
    fn extract_u64_works() {
        assert_eq!(extract_u64("(:id 42 :delta 100)", ":id"), Some(42));
        assert_eq!(extract_u64("(:id 42 :delta 100)", ":delta"), Some(100));
        assert_eq!(extract_u64("(:id 42)", ":missing"), None);
    }

    #[test]
    fn extract_f64_works() {
        assert_eq!(extract_f64("(:score 0.95)", ":score"), Some(0.95));
        assert_eq!(extract_f64("(:delta -0.5)", ":delta"), Some(-0.5));
    }

    #[test]
    fn extract_string_list_works() {
        let sexp = r#"(:tags ("rust" "actor" "ipc"))"#;
        assert_eq!(extract_string_list(sexp, ":tags"), vec!["rust", "actor", "ipc"]);
    }

    #[test]
    fn extract_bool_works() {
        assert_eq!(extract_bool("(:success t)", ":success"), Some(true));
        assert_eq!(extract_bool("(:success nil)", ":success"), Some(false));
    }
}
