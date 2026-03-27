//! Parallel-agents component dispatch.

use harmonia_actor_protocol::{extract_sexp_f64, extract_sexp_string, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => {
            // Already initialized via init_all
            "(:ok)".to_string()
        }
        "submit" => {
            let prompt = extract_sexp_string(sexp, ":prompt").unwrap_or_default();
            let model = extract_sexp_string(sexp, ":model").unwrap_or_default();
            match harmonia_parallel_agents::engine::submit(&prompt, &model) {
                Ok(task_id) => format!("(:ok :task-id {})", task_id),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "run-pending" => {
            let max = extract_sexp_string(sexp, ":max-parallel")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            match harmonia_parallel_agents::engine::run_pending(max) {
                Ok(()) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "run-pending-async" => {
            let max = extract_sexp_string(sexp, ":max-parallel")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            match harmonia_parallel_agents::engine::run_pending_async(max) {
                Ok(assignments) => {
                    let items: Vec<String> = assignments
                        .iter()
                        .map(|(tid, aid, model)| {
                            format!(
                                "(:task-id {} :actor-id {} :model \"{}\")",
                                tid,
                                aid,
                                esc(model)
                            )
                        })
                        .collect();
                    format!("(:ok :assignments ({}))", items.join(" "))
                }
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "task-result" => {
            let id = extract_sexp_string(sexp, ":task-id")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            match harmonia_parallel_agents::engine::task_result(id) {
                Ok(result) => format!("(:ok :result \"{}\")", esc(&result)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "report" => match harmonia_parallel_agents::engine::report() {
            Ok(r) => format!("(:ok :result \"{}\")", esc(&r)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "set-model-price" => {
            let model = extract_sexp_string(sexp, ":model").unwrap_or_default();
            let in_price = extract_sexp_f64(sexp, ":in-price").unwrap_or(0.0);
            let out_price = extract_sexp_f64(sexp, ":out-price").unwrap_or(0.0);
            let _ = harmonia_parallel_agents::engine::set_model_price(&model, in_price, out_price);
            "(:ok)".to_string()
        }
        _ => format!("(:error \"unknown parallel op: {}\")", op),
    }
}
