//! Vault component dispatch.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => {
            let rc = harmonia_vault::init_from_env();
            if rc.is_ok() {
                "(:ok)".to_string()
            } else {
                format!("(:error \"{}\")", esc(&format!("{:?}", rc.err())))
            }
        }
        "set-secret" => {
            let symbol = extract_sexp_string(sexp, ":symbol").unwrap_or_default();
            let value = extract_sexp_string(sexp, ":value").unwrap_or_default();
            match harmonia_vault::set_secret_for_symbol(&symbol, &value) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "has-secret" => {
            let symbol = extract_sexp_string(sexp, ":symbol").unwrap_or_default();
            if harmonia_vault::has_secret_for_symbol(&symbol) {
                "(:ok :result t)".to_string()
            } else {
                "(:ok :result nil)".to_string()
            }
        }
        "list-symbols" => {
            let symbols = harmonia_vault::list_secret_symbols();
            let items: Vec<String> = symbols.iter().map(|s| format!("\"{}\"", esc(s))).collect();
            format!("(:ok :symbols ({}))", items.join(" "))
        }
        _ => format!("(:error \"unknown vault op: {}\")", op),
    }
}
