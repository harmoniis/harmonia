//! Provider-router component dispatch — direct Rust API, no FFI.

use serde_json::json;

use harmonia_actor_protocol::extract_sexp_string;

use super::{dispatch_op, esc};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "complete" => {
            let prompt = extract_sexp_string(sexp, ":prompt").unwrap_or_default();
            let model = extract_sexp_string(sexp, ":model").unwrap_or_default();
            if harmonia_observability::harmonia_observability_is_standard() {
                let obs_ref = harmonia_observability::get_obs_actor().cloned();
                use harmonia_observability::Traceable;
                obs_ref.trace_event(
                    "provider-route",
                    "chain",
                    json!({"model": model.clone(), "op": "complete"}),
                );
            }
            dispatch_op!("complete",
                harmonia_provider_router::dispatch::route_complete(&prompt, &model)
                    .map(|text| format!("(:ok :result \"{}\")", esc(&text))))
        }
        "complete-for-task" => {
            let prompt = extract_sexp_string(sexp, ":prompt").unwrap_or_default();
            let task = extract_sexp_string(sexp, ":task").unwrap_or_default();
            dispatch_op!("complete-for-task",
                harmonia_provider_router::dispatch::route_complete_for_task(&prompt, &task)
                    .map(|text| format!("(:ok :result \"{}\")", esc(&text))))
        }
        "healthcheck" => {
            format!("(:ok :healthy t)")
        }
        "list-models" => {
            let result = harmonia_provider_router::status::list_models();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "select-model" => {
            let task = extract_sexp_string(sexp, ":task").unwrap_or_default();
            let result = harmonia_provider_router::status::select_model(&task);
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "list-backends" => {
            let result = harmonia_provider_router::status::all_backends_sexp();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "backend-status" => {
            let name = extract_sexp_string(sexp, ":name").unwrap_or_default();
            match harmonia_provider_router::status::backend_status_sexp(&name) {
                Some(sexp) => format!("(:ok :result \"{}\")", esc(&sexp)),
                None => format!("(:error \"unknown backend: {}\")", esc(&name)),
            }
        }
        _ => format!("(:error \"unknown provider-router op: {}\")", esc(&op)),
    }
}
