//! Config-store component dispatch.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => match harmonia_config_store::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "get" => {
            let component = extract_sexp_string(sexp, ":component").unwrap_or_default();
            let scope = extract_sexp_string(sexp, ":scope").unwrap_or_default();
            let key = extract_sexp_string(sexp, ":key").unwrap_or_default();
            match harmonia_config_store::get_config(&component, &scope, &key) {
                Ok(Some(v)) => format!("(:ok :value \"{}\")", esc(&v)),
                Ok(None) => "(:ok :value nil)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "get-or" => {
            let component = extract_sexp_string(sexp, ":component").unwrap_or_default();
            let scope = extract_sexp_string(sexp, ":scope").unwrap_or_default();
            let key = extract_sexp_string(sexp, ":key").unwrap_or_default();
            let default = extract_sexp_string(sexp, ":default").unwrap_or_default();
            match harmonia_config_store::get_config_or(&component, &scope, &key, &default) {
                Ok(v) => format!("(:ok :value \"{}\")", esc(&v)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "set" => {
            let component = extract_sexp_string(sexp, ":component").unwrap_or_default();
            let scope = extract_sexp_string(sexp, ":scope").unwrap_or_default();
            let key = extract_sexp_string(sexp, ":key").unwrap_or_default();
            let value = extract_sexp_string(sexp, ":value").unwrap_or_default();
            match harmonia_config_store::set_config(&component, &scope, &key, &value) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "list" => {
            let component = extract_sexp_string(sexp, ":component").unwrap_or_default();
            let scope = extract_sexp_string(sexp, ":scope").unwrap_or_default();
            match harmonia_config_store::list_scope(&component, &scope) {
                Ok(keys) => {
                    let items: Vec<String> =
                        keys.iter().map(|s| format!("\"{}\"", esc(s))).collect();
                    format!("(:ok :keys ({}))", items.join(" "))
                }
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "ingest-env" => match harmonia_config_store::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        _ => format!("(:error \"unknown config op: {}\")", op),
    }
}
