//! Vault component dispatch — actor-owned state, no singletons.

use super::{dispatch_op, esc, param, sexp_string_list};

/// Dispatch with actor-owned VaultState (preferred path).
pub(crate) fn dispatch_with_state(
    sexp: &str,
    state: &mut harmonia_vault::VaultState,
) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init",
            harmonia_vault::init_state(state).map(|_| "(:ok)".to_string())),
        "set-secret" => dispatch_op!("set-secret", {
            let (symbol, value) = (param!(sexp, ":symbol"), param!(sexp, ":value"));
            harmonia_vault::set_secret_with_state(state, &symbol, &value)
                .map(|_| "(:ok)".to_string())
        }),
        "has-secret" => {
            let symbol = param!(sexp, ":symbol");
            let has = harmonia_vault::has_secret_with_state(state, &symbol);
            format!("(:ok :result {})", if has { "t" } else { "nil" })
        }
        "list-symbols" => {
            let symbols = harmonia_vault::list_secrets_with_state(state);
            format!("(:ok :symbols ({}))", sexp_string_list(&symbols))
        }
        _ => format!("(:error \"unknown vault op: {}\")", esc(&op)),
    }
}

