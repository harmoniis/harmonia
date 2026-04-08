use std::sync::{Arc, RwLock};
use std::thread;

use harmonia_vault::{get_secret_for_component, init_from_env};

use crate::model::{append_metric_line, now_unix, sexp_escape, state, ModelPrice, Task};

use super::clients::request_openrouter;
use super::metrics::{estimate_cost, render_report};
use super::verification::verify_with_search;

pub fn set_model_price(
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

pub fn submit(prompt: &str, model: &str) -> Result<i64, String> {
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

pub fn run_pending(max_parallel: i32) -> Result<(), String> {
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

    let key = get_secret_for_component("openrouter-backend", "openrouter-api-key")
        .map_err(|e| format!("vault policy error: {e}"))?
        .or_else(|| {
            get_secret_for_component("openrouter-backend", "openrouter")
                .ok()
                .flatten()
        })
        .ok_or_else(|| {
            "missing secret: openrouter-api-key (vault component: openrouter-backend)".to_string()
        })?;

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

/// Run pending tasks asynchronously -- workers post results to the unified
/// actor mailbox when done instead of blocking on join.
///
/// Returns a list of `(task_id, actor_id, model)` tuples so the Lisp
/// supervisor can create tracking records BEFORE workers finish.
pub fn run_pending_async(max_parallel: i32) -> Result<Vec<(u64, u64, String)>, String> {
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
        return Ok(vec![]);
    }

    let key = get_secret_for_component("openrouter-backend", "openrouter-api-key")
        .map_err(|e| format!("vault policy error: {e}"))?
        .or_else(|| {
            get_secret_for_component("openrouter-backend", "openrouter")
                .ok()
                .flatten()
        })
        .ok_or_else(|| {
            "missing secret: openrouter-api-key (vault component: openrouter-backend)".to_string()
        })?;

    let limit = if max_parallel <= 0 {
        1usize
    } else {
        max_parallel as usize
    };

    // Actor registration now handled by harmonia-runtime via IPC.
    // Build placeholder assignments for return value compatibility.
    let actor_assignments: Vec<(u64, u64, String)> = pending
        .iter()
        .map(|t| (t.id, 0u64, t.model.clone()))
        .collect();

    let tasks = Arc::new(pending);
    let cursor = Arc::new(RwLock::new(0usize));

    // Mark all pending as running
    {
        let mut st = state()
            .write()
            .map_err(|_| "parallel state lock poisoned".to_string())?;
        for t in tasks.iter() {
            if let Some(task) = st.tasks.get_mut(&t.id) {
                task.status = "running".to_string();
            }
        }
    }

    for _ in 0..limit {
        let tasks = Arc::clone(&tasks);
        let cursor = Arc::clone(&cursor);
        let key = key.clone();

        // Each worker is a fire-and-forget thread
        thread::spawn(move || loop {
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

            // Update global state
            if let Ok(mut st) = state().write() {
                st.tasks.insert(t.id, t.clone());
                append_metric_line(&t);
            }

            // Actor mailbox posting now handled by harmonia-runtime via IPC.
        });
    }

    Ok(actor_assignments)
}

pub fn task_result(task_id: i64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "parallel state lock poisoned".to_string())?;
    let t = st
        .tasks
        .get(&(task_id as u64))
        .ok_or_else(|| "task not found".to_string())?;
    Ok(format!(
        "(:id {} :model \"{}\" :status :{} :success {} :verified {} :verification-source \"{}\" :verification-detail \"{}\" :latency-ms {} :cost-usd {:.8} :created-at {} :response \"{}\" :error \"{}\")",
        t.id,
        t.model,
        t.status,
        if t.success { "t" } else { "nil" },
        if t.verified { "t" } else { "nil" },
        t.verification_source,
        sexp_escape(&t.verification_detail),
        t.latency_ms,
        t.cost_usd,
        t.created_at,
        sexp_escape(&t.response),
        sexp_escape(&t.error)
    ))
}

pub fn report() -> Result<String, String> {
    Ok(render_report())
}
