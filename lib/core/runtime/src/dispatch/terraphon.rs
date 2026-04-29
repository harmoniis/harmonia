//! Terraphon component dispatch — requires actor-owned TerraphonState.

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};

use super::dispatch_op;

pub(crate) fn dispatch(
    sexp: &str,
    state: &mut harmonia_terraphon::TerraphonState,
) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init", harmonia_terraphon::init(state)),
        "health" => dispatch_op!("health", harmonia_terraphon::health_check(state)),
        "stats" => dispatch_op!("stats", harmonia_terraphon::stats(state)),
        "datamine" => {
            let lode_id = extract_sexp_string(sexp, ":lode-id").unwrap_or_default();
            let args_str = extract_sexp_string(sexp, ":args").unwrap_or_default();
            let args: Vec<&str> = args_str.split_whitespace().collect();
            dispatch_op!("datamine", harmonia_terraphon::datamine_local(state, &lode_id, &args))
        }
        "catalog" | "lodes" => dispatch_op!("catalog", harmonia_terraphon::catalog_list(state)),
        "lode-status" => {
            let lode_id = extract_sexp_string(sexp, ":lode-id").unwrap_or_default();
            dispatch_op!("lode-status", harmonia_terraphon::lode_status(state, &lode_id))
        }
        "plan" => {
            let domain = extract_sexp_string(sexp, ":domain").unwrap_or_else(|| "generic".into());
            let query = extract_sexp_string(sexp, ":query").unwrap_or_default();
            let strategy_str = extract_sexp_string(sexp, ":prefer").unwrap_or_else(|| "cascade".into());
            let strategy = harmonia_terraphon::QueryStrategy::from_str(&strategy_str);
            dispatch_op!("plan", harmonia_terraphon::plan_query(state, &domain, &query, strategy))
        }
        _ => format!("(:error \"unknown terraphon op: {}\")", sexp_escape(&op)),
    }
}
