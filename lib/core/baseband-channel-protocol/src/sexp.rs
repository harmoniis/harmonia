pub(crate) fn escape_sexp_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

pub(crate) fn sexp_string(value: &str) -> String {
    format!("\"{}\"", escape_sexp_string(value))
}

pub(crate) fn sexp_optional_string(key: &str, value: Option<&str>) -> String {
    match value {
        Some(v) if !v.is_empty() => format!(" :{} {}", key, sexp_string(v)),
        _ => String::new(),
    }
}

pub(crate) fn sexp_bool(value: bool) -> &'static str {
    if value {
        "t"
    } else {
        "nil"
    }
}
