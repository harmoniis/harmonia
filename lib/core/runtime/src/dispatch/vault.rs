//! Vault component dispatch — declarative operation table.

use super::{dispatch_op, param, sexp_string_list};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init",
            harmonia_vault::init_from_env().map(|_| "(:ok)".to_string())),
        "set-secret" => dispatch_op!("set-secret", {
            let (symbol, value) = (param!(sexp, ":symbol"), param!(sexp, ":value"));
            harmonia_vault::set_secret_for_symbol(&symbol, &value).map(|_| "(:ok)".to_string())
        }),
        "has-secret" => {
            let symbol = param!(sexp, ":symbol");
            let has = harmonia_vault::has_secret_for_symbol(&symbol);
            format!("(:ok :result {})", if has { "t" } else { "nil" })
        }
        "list-symbols" => {
            let symbols = harmonia_vault::list_secret_symbols();
            format!("(:ok :symbols ({}))", sexp_string_list(&symbols))
        }
        _ => format!("(:error \"unknown vault op: {}\")", op),
    }
}
