//! Provider-router component dispatch (FFI-based).

use serde_json::json;

use harmonia_actor_protocol::{extract_sexp_string, sexp_escape};

use super::{to_cstring, FfiString};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "complete" => {
            let prompt = extract_sexp_string(sexp, ":prompt").unwrap_or_default();
            let model = extract_sexp_string(sexp, ":model").unwrap_or_default();
            let prompt_c = match to_cstring(prompt.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let model_c = match to_cstring(model.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let model_ptr = if model.is_empty() {
                std::ptr::null()
            } else {
                model_c.as_ptr()
            };
            if harmonia_observability::harmonia_observability_is_standard() {
                let obs_ref = harmonia_observability::get_obs_actor().cloned();
                use harmonia_observability::Traceable;
                obs_ref.trace_event(
                    "provider-route",
                    "chain",
                    json!({"model": model.clone(), "op": "complete"}),
                );
            }
            let result_ptr = harmonia_provider_router::harmonia_provider_router_complete(
                prompt_c.as_ptr(),
                model_ptr,
            );
            let ffi_result = FfiString(result_ptr);
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "complete-for-task" => {
            let prompt = extract_sexp_string(sexp, ":prompt").unwrap_or_default();
            let task = extract_sexp_string(sexp, ":task").unwrap_or_default();
            let prompt_c = match to_cstring(prompt.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let task_c = match to_cstring(task.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let result_ptr = harmonia_provider_router::harmonia_provider_router_complete_for_task(
                prompt_c.as_ptr(),
                task_c.as_ptr(),
            );
            let ffi_result = FfiString(result_ptr);
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "healthcheck" => {
            let rc = harmonia_provider_router::harmonia_provider_router_healthcheck();
            format!("(:ok :healthy {})", if rc == 1 { "t" } else { "nil" })
        }
        "list-models" => {
            let ffi_result =
                FfiString(harmonia_provider_router::harmonia_provider_router_list_models());
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "select-model" => {
            let task = extract_sexp_string(sexp, ":task").unwrap_or_default();
            let task_c = match to_cstring(task.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let ffi_result = FfiString(
                harmonia_provider_router::harmonia_provider_router_select_model(task_c.as_ptr()),
            );
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "list-backends" => {
            let ffi_result =
                FfiString(harmonia_provider_router::harmonia_provider_router_list_backends());
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        "backend-status" => {
            let name = extract_sexp_string(sexp, ":name").unwrap_or_default();
            let name_c = match to_cstring(name.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let ffi_result = FfiString(
                harmonia_provider_router::harmonia_provider_router_backend_status(name_c.as_ptr()),
            );
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        _ => format!("(:error \"unknown provider-router op: {}\")", op),
    }
}
