mod clients;
mod metrics;

use std::sync::{Arc, RwLock};
use std::thread;

use harmonia_vault::{get_secret_for_component, init_from_env};

use crate::model::{
    append_metric_line, clear_error, json_escape, now_unix, set_error, state,
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

    let key = get_secret_for_component("parallel-agents-core", "openrouter")
        .map_err(|e| format!("vault policy error: {e}"))?
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
    Ok(render_report())
}

// ---------------------------------------------------------------------------
// Tmux CLI agent engine functions — distinct operational tier
// ---------------------------------------------------------------------------

use crate::model::CliType;
use crate::tmux::controller;

pub(crate) fn tmux_spawn(cli_type_str: &str, workdir: &str, prompt: &str) -> Result<i64, String> {
    let cli_type = CliType::from_str(cli_type_str)?;
    controller::spawn(&cli_type, workdir, prompt).map(|id| id as i64)
}

pub(crate) fn tmux_spawn_custom(
    command: &str,
    args: &str,
    workdir: &str,
    prompt: &str,
) -> Result<i64, String> {
    let shell_args: Vec<String> = if args.is_empty() {
        vec![]
    } else {
        args.split_whitespace().map(|s| s.to_string()).collect()
    };
    let cli_type = CliType::Custom {
        command: command.to_string(),
        shell_args,
    };
    controller::spawn(&cli_type, workdir, prompt).map(|id| id as i64)
}

pub(crate) fn tmux_poll(id: i64) -> Result<String, String> {
    let state = controller::poll(id as u64)?;
    Ok(state.to_sexp())
}

pub(crate) fn tmux_send(id: i64, input: &str) -> Result<(), String> {
    controller::send_input(id as u64, input)
}

pub(crate) fn tmux_send_key(id: i64, key: &str) -> Result<(), String> {
    controller::send_key(id as u64, key)
}

pub(crate) fn tmux_approve(id: i64) -> Result<(), String> {
    controller::approve(id as u64)
}

pub(crate) fn tmux_deny(id: i64) -> Result<(), String> {
    controller::deny(id as u64)
}

pub(crate) fn tmux_confirm_yes(id: i64) -> Result<(), String> {
    controller::confirm_yes(id as u64)
}

pub(crate) fn tmux_confirm_no(id: i64) -> Result<(), String> {
    controller::confirm_no(id as u64)
}

pub(crate) fn tmux_select(id: i64, index: i32) -> Result<(), String> {
    controller::select_option(id as u64, index as usize)
}

pub(crate) fn tmux_capture(id: i64, history: i32) -> Result<String, String> {
    let h = if history <= 0 { 200 } else { history as u32 };
    controller::capture(id as u64, h)
}

pub(crate) fn tmux_kill(id: i64) -> Result<(), String> {
    controller::kill(id as u64)
}

pub(crate) fn tmux_interrupt(id: i64) -> Result<(), String> {
    controller::interrupt(id as u64)
}

pub(crate) fn tmux_status(id: i64) -> Result<String, String> {
    controller::agent_status(id as u64)
}

pub(crate) fn tmux_list() -> Result<String, String> {
    controller::list()
}

pub(crate) fn tmux_swarm_poll() -> Result<String, String> {
    controller::swarm_poll()
}

// ---------------------------------------------------------------------------
// Core infrastructure
// ---------------------------------------------------------------------------

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
