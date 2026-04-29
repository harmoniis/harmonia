//! Chronicle component dispatch — split into query and record sub-modules.
//! Each sub-module handles a coherent group of operations.

mod query;
mod record;

use super::{dispatch_op, esc};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();

    // Lifecycle ops handled here (small group)
    match op.as_str() {
        "init" => return dispatch_op!("init",
            harmonia_chronicle::init().map(|_| "(:ok)".to_string())),
        "gc" => return dispatch_op!("gc",
            harmonia_chronicle::gc().map(|n| format!("(:ok :result \"{}\")", n))),
        _ => {}
    }

    // Delegate to query or record sub-module
    if let Some(result) = query::dispatch(&op, sexp) {
        return result;
    }
    if let Some(result) = record::dispatch(&op, sexp) {
        return result;
    }

    format!("(:error \"unknown chronicle op: {}\")", esc(&op))
}
