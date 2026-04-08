//! Config-store component dispatch — pure functional, declarative.

use super::{dispatch_op, param, sexp_string_list};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init",
            harmonia_config_store::init().map(|_| "(:ok)".to_string())),
        "get" => {
            let (component, scope, key) = (param!(sexp, ":component"), param!(sexp, ":scope"), param!(sexp, ":key"));
            dispatch_op!("get", harmonia_config_store::get_config(&component, &scope, &key).map(|v|
                v.map_or("(:ok :value nil)".to_string(),
                    |val| format!("(:ok :value \"{}\")", harmonia_actor_protocol::sexp_escape(&val)))))
        }
        "get-or" => {
            let (component, scope, key, default) = (param!(sexp, ":component"), param!(sexp, ":scope"), param!(sexp, ":key"), param!(sexp, ":default"));
            dispatch_op!("get-or", harmonia_config_store::get_config_or(&component, &scope, &key, &default)
                .map(|v| format!("(:ok :value \"{}\")", harmonia_actor_protocol::sexp_escape(&v))))
        }
        "set" => {
            let (component, scope, key, value) = (param!(sexp, ":component"), param!(sexp, ":scope"), param!(sexp, ":key"), param!(sexp, ":value"));
            dispatch_op!("set", harmonia_config_store::set_config(&component, &scope, &key, &value)
                .map(|_| "(:ok)".to_string()))
        }
        "list" => {
            let (component, scope) = (param!(sexp, ":component"), param!(sexp, ":scope"));
            dispatch_op!("list", harmonia_config_store::list_scope(&component, &scope)
                .map(|keys| format!("(:ok :keys ({}))", sexp_string_list(&keys))))
        }
        "ingest-env" => dispatch_op!("ingest-env",
            harmonia_config_store::init().map(|_| "(:ok)".to_string())),
        _ => format!("(:error \"unknown config op: {}\")", op),
    }
}
