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

/// Truncate a string to at most `max_bytes` bytes at a valid UTF-8 boundary.
pub fn truncate_safe(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes { return s; }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) { end -= 1; }
    &s[..end]
}

/// Clamp a float to the range [lo, hi].
pub fn clamp_f64(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

/// Declarative macro for sexp-serializable enums.
/// Generates enum + to_sexp/try_from_sexp/from_str methods.
#[macro_export]
macro_rules! define_sexp_enum {
    ($name:ident, $default:ident { $($variant:ident => $kw:literal),* $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $($variant),* }
        impl $name {
            pub fn to_sexp(&self) -> &'static str {
                match self { $(Self::$variant => concat!(":", $kw)),* }
            }
            pub fn try_from_sexp(s: &str) -> Option<Self> {
                let s = s.strip_prefix(':').unwrap_or(s);
                match s { $($kw => Some(Self::$variant),)* _ => None, }
            }
            pub fn from_str(s: &str) -> Self {
                Self::try_from_sexp(s).unwrap_or(Self::$default)
            }
        }
    };
}

// ── SexpBuilder: declarative sexp construction ──

/// Builder for constructing sexp strings declaratively.
/// Replaces error-prone `format!` patterns with typed construction.
///
/// ```
/// use harmonia_actor_protocol::SexpBuilder;
/// let s = SexpBuilder::ok().key("n").uint(5).key("edges").uint(10).build();
/// assert_eq!(s, "(:ok :n 5 :edges 10)");
/// ```
pub struct SexpBuilder {
    buf: String,
}

impl SexpBuilder {
    /// Start an `:ok` sexp.
    pub fn ok() -> Self {
        Self { buf: "(:ok".into() }
    }

    /// Start an `:error` sexp.
    pub fn error() -> Self {
        Self { buf: "(:error".into() }
    }

    /// Append a keyword (`:key`).
    pub fn key(mut self, k: &str) -> Self {
        self.buf.push_str(" :");
        self.buf.push_str(k);
        self
    }

    /// Append a signed integer value.
    pub fn int(mut self, v: i64) -> Self {
        self.buf.push(' ');
        self.buf.push_str(&v.to_string());
        self
    }

    /// Append an unsigned integer value.
    pub fn uint(mut self, v: u64) -> Self {
        self.buf.push(' ');
        self.buf.push_str(&v.to_string());
        self
    }

    /// Append a float value with specified decimal places.
    pub fn float(mut self, v: f64, decimals: usize) -> Self {
        self.buf.push(' ');
        self.buf.push_str(&format!("{:.prec$}", v, prec = decimals));
        self
    }

    /// Append an escaped, quoted string value.
    pub fn str(mut self, v: &str) -> Self {
        self.buf.push_str(" \"");
        self.buf.push_str(&escape(v));
        self.buf.push('"');
        self
    }

    /// Append a raw (unquoted) value. Use for pre-formatted sexp fragments.
    pub fn raw(mut self, v: &str) -> Self {
        self.buf.push(' ');
        self.buf.push_str(v);
        self
    }

    /// Append a parenthesized list of pre-formatted items.
    pub fn list(mut self, items: &[String]) -> Self {
        self.buf.push_str(" (");
        self.buf.push_str(&items.join(" "));
        self.buf.push(')');
        self
    }

    /// Finalize and return the sexp string.
    pub fn build(mut self) -> String {
        self.buf.push(')');
        self.buf
    }
}

// ── Sexp tree parser + plist accessors ──
//
// String extractors above operate on raw text; the tree parser below builds an
// AST so callers can do nested plist access (`:harmony (:logistic-r-delta …)`)
// without re-scanning the string. ONE parser for the whole codebase.

#[derive(Debug, Clone)]
pub enum Sexp {
    List(Vec<Sexp>),
    Atom(String),
    String(String),
}

struct SexpParser<'a> {
    chars: Vec<char>,
    index: usize,
    _raw: &'a str,
}

pub fn parse_sexp(raw: &str) -> Result<Sexp, String> {
    let mut parser = SexpParser::new(raw);
    let sexp = parser.parse_expr()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err("unexpected trailing content".to_string());
    }
    Ok(sexp)
}

impl<'a> SexpParser<'a> {
    fn new(raw: &'a str) -> Self {
        Self { chars: raw.chars().collect(), index: 0, _raw: raw }
    }
    fn peek(&self) -> Option<char> { self.chars.get(self.index).copied() }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        Some(ch)
    }
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
            self.index += 1;
        }
    }
    fn parse_expr(&mut self) -> Result<Sexp, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => self.parse_list(),
            Some('"') => self.parse_string(),
            Some(_) => self.parse_atom(),
            None => Err("unexpected end of input".to_string()),
        }
    }
    fn parse_list(&mut self) -> Result<Sexp, String> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(')') => { self.bump(); return Ok(Sexp::List(items)); }
                Some(_) => items.push(self.parse_expr()?),
                None => return Err("unterminated list".to_string()),
            }
        }
    }
    fn parse_string(&mut self) -> Result<Sexp, String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(Sexp::String(out)),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => return Err("unterminated escape".to_string()),
                },
                Some(ch) => out.push(ch),
                None => return Err("unterminated string".to_string()),
            }
        }
    }
    fn parse_atom(&mut self) -> Result<Sexp, String> {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || ch == '(' || ch == ')' { break; }
            out.push(ch);
            self.index += 1;
        }
        if out.is_empty() { Err("expected atom".to_string()) } else { Ok(Sexp::Atom(out)) }
    }
}

trait KeywordAtom {
    fn atom_starts_with_keyword(&self) -> bool;
}
impl KeywordAtom for Sexp {
    fn atom_starts_with_keyword(&self) -> bool {
        matches!(self, Sexp::Atom(atom) if atom.starts_with(':'))
    }
}

pub fn plist_view(sexp: &Sexp) -> Result<&[Sexp], String> {
    match sexp {
        Sexp::List(items) => {
            if items.is_empty() {
                return Ok(items);
            }
            if let Sexp::Atom(atom) = &items[0] {
                if atom.starts_with(':')
                    && items.len() > 1
                    && matches!(items[1], Sexp::Atom(_))
                    && items[1].atom_starts_with_keyword()
                {
                    return Ok(&items[1..]);
                }
            }
            Ok(items)
        }
        _ => Err("expected plist list".to_string()),
    }
}

pub fn plist_value<'a>(items: &'a [Sexp], key: &str) -> Option<&'a Sexp> {
    let needle = format!(":{key}");
    let mut index = 0;
    while index + 1 < items.len() {
        if let Sexp::Atom(atom) = &items[index] {
            if atom.eq_ignore_ascii_case(&needle) {
                return items.get(index + 1);
            }
        }
        index += 2;
    }
    None
}

pub fn plist_list<'a>(items: &'a [Sexp], key: &str) -> Option<&'a [Sexp]> {
    match plist_value(items, key) {
        Some(Sexp::List(list)) => Some(list.as_slice()),
        _ => None,
    }
}

pub fn plist_f64(items: &[Sexp], key: &str) -> Option<f64> {
    plist_value(items, key).and_then(sexp_to_f64)
}
pub fn plist_i64(items: &[Sexp], key: &str) -> Option<i64> {
    plist_value(items, key).and_then(sexp_to_i64)
}
pub fn plist_bool(items: &[Sexp], key: &str) -> Option<bool> {
    plist_value(items, key).and_then(sexp_to_bool)
}
pub fn plist_string(items: &[Sexp], key: &str) -> Option<String> {
    plist_value(items, key).and_then(sexp_to_string_value)
}

pub fn sexp_to_f64(sexp: &Sexp) -> Option<f64> {
    match sexp {
        Sexp::Atom(atom) => atom.parse::<f64>().ok(),
        Sexp::String(text) => text.parse::<f64>().ok(),
        Sexp::List(_) => None,
    }
}
pub fn sexp_to_i64(sexp: &Sexp) -> Option<i64> {
    match sexp {
        Sexp::Atom(atom) => atom
            .parse::<i64>()
            .ok()
            .or_else(|| atom.parse::<f64>().ok().map(|value| value.round() as i64)),
        Sexp::String(text) => text.parse::<i64>().ok(),
        Sexp::List(_) => None,
    }
}
pub fn sexp_to_bool(sexp: &Sexp) -> Option<bool> {
    match sexp {
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case("t") => Some(true),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case("nil") => Some(false),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case(":true") => Some(true),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case(":false") => Some(false),
        Sexp::String(text) if text.eq_ignore_ascii_case("true") => Some(true),
        Sexp::String(text) if text.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}
pub fn sexp_to_string_value(sexp: &Sexp) -> Option<String> {
    match sexp {
        Sexp::Atom(atom) => Some(atom.clone()),
        Sexp::String(text) => Some(text.clone()),
        Sexp::List(_) => None,
    }
}

pub fn parse_number_list(sexp: &Sexp) -> Result<Vec<f64>, String> {
    match sexp {
        Sexp::List(items) => items
            .iter()
            .map(|item| sexp_to_f64(item).ok_or_else(|| "expected numeric atom".to_string()))
            .collect(),
        _ => Err("expected list".to_string()),
    }
}

pub fn parse_fixed_array<const N: usize>(
    sexp: Option<&Sexp>,
    label: &str,
) -> Result<[f64; N], String> {
    let values = parse_vector_exact(sexp, N, label)?;
    let mut output = [0.0; N];
    for (slot, value) in output.iter_mut().zip(values.iter()) {
        *slot = *value;
    }
    Ok(output)
}

pub fn parse_vector_exact(
    sexp: Option<&Sexp>,
    expected: usize,
    label: &str,
) -> Result<Vec<f64>, String> {
    let values = parse_number_list(sexp.ok_or_else(|| format!("missing {label}"))?)?;
    if values.len() != expected {
        return Err(format!(
            "invalid {label}: expected {expected} values, got {}",
            values.len()
        ));
    }
    Ok(values)
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

    #[test]
    fn sexp_builder_ok_with_key_values() {
        let s = SexpBuilder::ok().key("n").uint(5).key("edges").uint(10).build();
        assert_eq!(s, "(:ok :n 5 :edges 10)");
    }

    #[test]
    fn sexp_builder_error_with_string() {
        let s = SexpBuilder::error().str("something failed").build();
        assert_eq!(s, r#"(:error "something failed")"#);
    }

    #[test]
    fn sexp_builder_float_precision() {
        let s = SexpBuilder::ok().key("score").float(0.123456, 3).build();
        assert_eq!(s, "(:ok :score 0.123)");
    }

    #[test]
    fn sexp_builder_list() {
        let items = vec!["0.1234".to_string(), "0.5678".to_string()];
        let s = SexpBuilder::ok().key("eigenvalues").list(&items).build();
        assert_eq!(s, "(:ok :eigenvalues (0.1234 0.5678))");
    }

    #[test]
    fn sexp_builder_escapes_strings() {
        let s = SexpBuilder::ok().key("label").str(r#"hello "world""#).build();
        assert_eq!(s, r#"(:ok :label "hello \"world\"")"#);
    }

    #[test]
    fn sexp_builder_raw_fragment() {
        let s = SexpBuilder::ok().key("basin").raw(":thomas-0").build();
        assert_eq!(s, "(:ok :basin :thomas-0)");
    }
}
