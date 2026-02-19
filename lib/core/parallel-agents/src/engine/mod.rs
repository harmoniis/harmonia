mod clients;
mod metrics;

use std::fs;
use std::sync::{Arc, RwLock};
use std::thread;

use harmonia_vault::{get_secret_for_symbol, init_from_env};

use crate::model::{
    append_metric_line, clear_error, json_escape, metrics_log_path, now_unix, set_error, state,
    ModelPrice, Task,
};

use self::clients::{request_openrouter, verify_with_search};
use self::metrics::{estimate_cost, render_report};

pub(crate) fn init_backend() -> Result<(), String> {
    init_from_env().map_err(|e| e.to_string())
}

pub(crate) fn set_model_price(
    model: &str,
    usd_per_1k_input: f64,
    usd_per_1k_output: f64,
) -> Result<(), String> {
    match state().write() {
        Ok(mut st) => {
            st.prices.insert(
                model.to_string(),
                ModelPrice {
                    usd_per_1k_input,
                    usd_per_1k_output,
                },
            );
            Ok(())
        }
        Err(_) => Err("parallel state lock poisoned".to_string()),
    }
}

pub(crate) fn submit(prompt: &str, model: &str) -> Result<i64, String> {
    let mut st = state()
        .write()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let id = st.next_id;
    st.next_id += 1;
    st.tasks.insert(
        id,
        Task {
            id,
            prompt: prompt.to_string(),
            model: model.to_string(),
            status: "pending".to_string(),
            response: String::new(),
            error: String::new(),
            latency_ms: 0,
            cost_usd: 0.0,
            success: false,
            verified: false,
            verification_source: "none".to_string(),
            verification_detail: String::new(),
            created_at: now_unix(),
        },
    );
    Ok(id as i64)
}

pub(crate) fn run_pending(max_parallel: i32) -> Result<(), String> {
    init_from_env().map_err(|e| e.to_string())?;

    let pending: Vec<Task> = {
        let st = state()
            .read()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        st.tasks
            .values()
            .filter(|t| t.status == "pending")
            .cloned()
            .collect()
    };

    if pending.is_empty() {
        return Ok(());
    }

    let key = get_secret_for_symbol("openrouter")
        .ok_or_else(|| "missing secret: openrouter".to_string())?;

    let limit = if max_parallel <= 0 {
        1usize
    } else {
        max_parallel as usize
    };
    let tasks = Arc::new(pending);
    let cursor = Arc::new(RwLock::new(0usize));
    let results: Arc<RwLock<Vec<Task>>> = Arc::new(RwLock::new(Vec::new()));

    let mut workers = Vec::new();
    for _ in 0..limit {
        let tasks = Arc::clone(&tasks);
        let cursor = Arc::clone(&cursor);
        let results = Arc::clone(&results);
        let key = key.clone();
        workers.push(thread::spawn(move || loop {
            let next = {
                let mut idx = match cursor.write() {
                    Ok(v) => v,
                    Err(_) => return,
                };
                if *idx >= tasks.len() {
                    None
                } else {
                    let i = *idx;
                    *idx += 1;
                    Some(i)
                }
            };
            let i = match next {
                Some(v) => v,
                None => break,
            };
            let mut t = tasks[i].clone();
            t.status = "running".to_string();
            let start = std::time::Instant::now();
            match request_openrouter(&t.prompt, &t.model, &key) {
                Ok(resp) => {
                    t.response = resp;
                    t.success = true;
                    let (verified, source, detail) = verify_with_search(&t.prompt, &t.response);
                    t.verified = verified;
                    t.verification_source = source;
                    t.verification_detail = detail;
                    t.status = "done".to_string();
                }
                Err(e) => {
                    t.error = e;
                    t.success = false;
                    t.verified = false;
                    t.verification_source = "none".to_string();
                    t.verification_detail = "openrouter-error".to_string();
                    t.status = "error".to_string();
                }
            }
            t.latency_ms = start.elapsed().as_millis() as u64;
            t.cost_usd = if t.success {
                estimate_cost(&t.model, &t.prompt, &t.response)
            } else {
                0.0
            };
            if let Ok(mut r) = results.write() {
                r.push(t);
            }
        }));
    }

    for w in workers {
        let _ = w.join();
    }

    let done = results
        .read()
        .map_err(|_| "parallel result lock poisoned".to_string())?
        .clone();

    let mut st = state()
        .write()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    for t in done {
        st.tasks.insert(t.id, t.clone());
        append_metric_line(&t);
    }

    Ok(())
}

pub(crate) fn task_result(task_id: i64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let t = st
        .tasks
        .get(&(task_id as u64))
        .ok_or_else(|| "task not found".to_string())?;
    Ok(format!(
        "(:id {} :model \"{}\" :status :{} :success {} :verified {} :verification-source \"{}\" :verification-detail \"{}\" :latency-ms {} :cost-usd {:.8} :response \"{}\" :error \"{}\")",
        t.id,
        t.model,
        t.status,
        if t.success { "t" } else { "nil" },
        if t.verified { "t" } else { "nil" },
        t.verification_source,
        json_escape(&t.verification_detail),
        t.latency_ms,
        t.cost_usd,
        json_escape(&t.response),
        json_escape(&t.error)
    ))
}

pub(crate) fn report() -> Result<String, String> {
    let path = metrics_log_path();
    let content = fs::read_to_string(path).unwrap_or_default();
    Ok(render_report(&content))
}

pub(crate) fn healthcheck() -> i32 {
    1
}

pub(crate) fn init_ffi() -> i32 {
    match init_backend() {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}
