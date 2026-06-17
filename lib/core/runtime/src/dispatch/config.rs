//! Config-store component dispatch — pure functional, declarative.

use super::{dispatch_op, esc, param, sexp_string_list};

pub(crate) fn dispatch(sexp: &str) -> String {
    // Routing target is ":component" ("config"); the CALLER identity that the policy gates on is a
    // distinct ":caller" key — otherwise extract_string's first-match would read the routing
    // "config" as the caller and every non-"config" scope read/write would be denied.
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init",
            harmonia_config_store::init().map(|_| "(:ok)".to_string())),
        "get" => {
            let (caller, scope, key) = (param!(sexp, ":caller"), param!(sexp, ":scope"), param!(sexp, ":key"));
            dispatch_op!("get", harmonia_config_store::get_config(&caller, &scope, &key).map(|v|
                v.map_or("(:ok :result nil)".to_string(),
                    |val| format!("(:ok :result \"{}\")", harmonia_actor_protocol::sexp_escape(&val)))))
        }
        "get-or" => {
            let (caller, scope, key, default) = (param!(sexp, ":caller"), param!(sexp, ":scope"), param!(sexp, ":key"), param!(sexp, ":default"));
            dispatch_op!("get-or", harmonia_config_store::get_config_or(&caller, &scope, &key, &default)
                .map(|v| format!("(:ok :result \"{}\")", harmonia_actor_protocol::sexp_escape(&v))))
        }
        "set" => {
            let (caller, scope, key, value) = (param!(sexp, ":caller"), param!(sexp, ":scope"), param!(sexp, ":key"), param!(sexp, ":value"));
            dispatch_op!("set", harmonia_config_store::set_config(&caller, &scope, &key, &value)
                .map(|_| "(:ok)".to_string()))
        }
        "list" => {
            let (caller, scope) = (param!(sexp, ":caller"), param!(sexp, ":scope"));
            dispatch_op!("list", harmonia_config_store::list_scope(&caller, &scope)
                .map(|keys| format!("(:ok :keys ({}))", sexp_string_list(&keys))))
        }
        "ingest-env" => dispatch_op!("ingest-env",
            harmonia_config_store::init().map(|_| "(:ok)".to_string())),
        _ => format!("(:error \"unknown config op: {}\")", esc(&op)),
    }
}
