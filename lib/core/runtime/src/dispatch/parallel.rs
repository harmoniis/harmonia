//! Parallel-agents component dispatch — pure functional, declarative.

use super::{dispatch_op, esc, param, param_f64};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => "(:ok)".to_string(),
        "submit" => dispatch_op!("submit", {
            let (prompt, model) = (param!(sexp, ":prompt"), param!(sexp, ":model"));
            harmonia_parallel_agents::engine::submit(&prompt, &model)
                .map(|task_id| format!("(:ok :task-id {})", task_id))
        }),
        "run-pending" => dispatch_op!("run-pending", {
            let max = harmonia_actor_protocol::extract_sexp_string(sexp, ":max-parallel")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            harmonia_parallel_agents::engine::run_pending(max)
                .map(|_| "(:ok)".to_string())
        }),
        "run-pending-async" => dispatch_op!("run-pending-async", {
            let max = harmonia_actor_protocol::extract_sexp_string(sexp, ":max-parallel")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            harmonia_parallel_agents::engine::run_pending_async(max)
                .map(|assignments| {
                    let items: String = assignments.iter()
                        .map(|(tid, aid, model)| format!(
                            "(:task-id {} :actor-id {} :model \"{}\")",
                            tid, aid, esc(model),
                        ))
                        .collect::<Vec<_>>()
                        .join(" ");
                    format!("(:ok :assignments ({}))", items)
                })
        }),
        "task-result" => dispatch_op!("task-result", {
            let id = harmonia_actor_protocol::extract_sexp_string(sexp, ":task-id")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            harmonia_parallel_agents::engine::task_result(id)
                .map(|result| format!("(:ok :result \"{}\")", esc(&result)))
        }),
        "report" => dispatch_op!("report",
            harmonia_parallel_agents::engine::report()
                .map(|r| format!("(:ok :result \"{}\")", esc(&r)))),
        "set-model-price" => {
            let model = param!(sexp, ":model");
            let in_price = param_f64!(sexp, ":in-price", 0.0);
            let out_price = param_f64!(sexp, ":out-price", 0.0);
            let _ = harmonia_parallel_agents::engine::set_model_price(&model, in_price, out_price);
            "(:ok)".to_string()
        }
        _ => format!("(:error \"unknown parallel op: {}\")", esc(&op)),
    }
}
