//! Chronicle query operations — read-only data retrieval.

use super::super::{dispatch_op, esc, param};

pub(crate) fn dispatch(op: &str, sexp: &str) -> Option<String> {
    Some(match op {
        "query" => dispatch_op!("query", {
            let sql = param!(sexp, ":sql");
            harmonia_chronicle::query_sexp(&sql)
                .map(|result| format!("(:ok :result \"{}\")", esc(&result)))
        }),
        "harmony-summary" => dispatch_op!("harmony-summary",
            harmonia_chronicle::harmony_summary()
                .map(|s| format!("(:ok :result \"{}\")", esc(&s)))),
        "dashboard" => dispatch_op!("dashboard",
            harmonia_chronicle::dashboard_json()
                .map(|s| format!("(:ok :result \"{}\")", esc(&s)))),
        "gc-status" => dispatch_op!("gc-status",
            harmonia_chronicle::gc_status()
                .map(|s| format!("(:ok :result \"{}\")", esc(&s)))),
        "cost-report" => dispatch_op!("cost-report",
            harmonia_chronicle::cost_report(0)
                .map(|s| format!("(:ok :result \"{}\")", esc(&s)))),
        "delegation-report" => dispatch_op!("delegation-report",
            harmonia_chronicle::delegation_report()
                .map(|s| format!("(:ok :result \"{}\")", esc(&s)))),
        "full-digest" => dispatch_op!("full-digest",
            harmonia_chronicle::full_digest()
                .map(|s| format!("(:ok :result \"{}\")", esc(&s)))),
        "load-all-entries" => dispatch_op!("load-all-entries",
            harmonia_chronicle::memory::load_all_entries()),
        "entry-count" => dispatch_op!("entry-count",
            harmonia_chronicle::memory::entry_count()
                .map(|count| format!("(:ok :count {})", count))),
        _ => return None,
    })
}
