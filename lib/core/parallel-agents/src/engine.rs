use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::{Arc, RwLock};
use std::thread;

use harmonia_vault::{get_secret_for_symbol, init_from_env};

use crate::model::{
    append_metric_line, clear_error, json_escape, metrics_log_path, now_unix, set_error, state,
    ModelPrice, Task,
};

fn extract_content_from_response(payload: &str) -> Option<String> {
    let key = "\"content\":\"";
    let start = payload.find(key)?;
    let rest = &payload[start + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_error_message(payload: &str) -> Option<String> {
    let key = "\"message\":\"";
    let start = payload.find(key)?;
    let rest = &payload[start + key.len()..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn request_openrouter(prompt: &str, model: &str, api_key: &str) -> Result<String, String> {
    let payload = format!(
        "{{\"model\":\"{}\",\"messages\":[{{\"role\":\"user\",\"content\":\"{}\"}}]}}",
        json_escape(model),
        json_escape(prompt)
    );

    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg(
            env::var("HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS")
                .unwrap_or_else(|_| "10".to_string()),
        )
        .arg("--max-time")
        .arg(env::var("HARMONIA_OPENROUTER_MAX_TIME_SECS").unwrap_or_else(|_| "45".to_string()))
        .arg("-X")
        .arg("POST")
        .arg("https://openrouter.ai/api/v1/chat/completions")
        .arg("-H")
        .arg(format!("Authorization: Bearer {api_key}"))
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg("HTTP-Referer: https://harmoniis.local")
        .arg("-H")
        .arg("X-Title: Harmonia Parallel Agents")
        .arg("-d")
        .arg(payload)
        .output()
        .map_err(|e| format!("curl exec failed: {e}"))?;

    if !out.status.success() {
        return Err(format!(
            "curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if let Some(e) = extract_error_message(&stdout) {
        return Err(e);
    }
    extract_content_from_response(&stdout)
        .ok_or_else(|| format!("missing content in response: {stdout}"))
}

fn request_exa(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = env::var("HARMONIA_EXA_API_URL")
        .unwrap_or_else(|_| "https://api.exa.ai/search".to_string());
    let payload = format!("{{\"query\":\"{}\",\"numResults\":5}}", json_escape(query));
    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-X")
        .arg("POST")
        .arg("-H")
        .arg("Content-Type: application/json")
        .arg("-H")
        .arg(format!("x-api-key: {api_key}"))
        .arg("-d")
        .arg(payload)
        .arg(endpoint)
        .output()
        .map_err(|e| format!("exa curl exec failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "exa curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn request_brave(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = env::var("HARMONIA_BRAVE_API_URL")
        .unwrap_or_else(|_| "https://api.search.brave.com/res/v1/web/search".to_string());
    let out = std::process::Command::new("curl")
        .arg("-sS")
        .arg("-G")
        .arg("-H")
        .arg(format!("X-Subscription-Token: {api_key}"))
        .arg("--data-urlencode")
        .arg(format!("q={query}"))
        .arg(endpoint)
        .output()
        .map_err(|e| format!("brave curl exec failed: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "brave curl failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn make_verify_query(prompt: &str, response: &str) -> String {
    let p = prompt.chars().take(120).collect::<String>();
    let r = response.chars().take(180).collect::<String>();
    format!("verify this answer against the web: prompt={p} answer={r}")
}

fn verify_with_search(prompt: &str, response: &str) -> (bool, String, String) {
    let _ = init_from_env();
    if response.trim().is_empty() {
        return (false, "none".to_string(), "empty-response".to_string());
    }
    let q = make_verify_query(prompt, response);

    if let Some(exa) = get_secret_for_symbol("exa_api_key") {
        match request_exa(&q, &exa) {
            Ok(payload) => {
                let ok = payload.contains("\"results\"");
                return (
                    ok,
                    "exa".to_string(),
                    if ok {
                        "exa-results-found".to_string()
                    } else {
                        "exa-no-results".to_string()
                    },
                );
            }
            Err(e) => {
                if let Some(brave) = get_secret_for_symbol("brave_api_key") {
                    match request_brave(&q, &brave) {
                        Ok(payload) => {
                            let ok = payload.contains("\"web\"") || payload.contains("\"results\"");
                            return (
                                ok,
                                "brave".to_string(),
                                if ok {
                                    "brave-results-found".to_string()
                                } else {
                                    "brave-no-results".to_string()
                                },
                            );
                        }
                        Err(e2) => {
                            return (false, "none".to_string(), format!("exa={e}; brave={e2}"));
                        }
                    }
                }
                return (
                    false,
                    "none".to_string(),
                    format!("exa={e}; brave=missing-key"),
                );
            }
        }
    }

    if let Some(brave) = get_secret_for_symbol("brave_api_key") {
        match request_brave(&q, &brave) {
            Ok(payload) => {
                let ok = payload.contains("\"web\"") || payload.contains("\"results\"");
                return (
                    ok,
                    "brave".to_string(),
                    if ok {
                        "brave-results-found".to_string()
                    } else {
                        "brave-no-results".to_string()
                    },
                );
            }
            Err(e) => return (false, "none".to_string(), format!("brave={e}")),
        }
    }

    (false, "none".to_string(), "missing-search-keys".to_string())
}

fn estimate_tokens(s: &str) -> f64 {
    (s.chars().count() as f64 / 4.0).max(1.0)
}

fn estimate_cost(model: &str, prompt: &str, response: &str) -> f64 {
    let st = match state().read() {
        Ok(s) => s,
        Err(_) => return 0.0,
    };
    let p = match st.prices.get(model) {
        Some(v) => *v,
        None => return 0.0,
    };
    let in_k = estimate_tokens(prompt) / 1000.0;
    let out_k = estimate_tokens(response) / 1000.0;
    p.usd_per_1k_input * in_k + p.usd_per_1k_output * out_k
}

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
    let mut total = 0u64;
    let mut success = 0u64;
    let mut verified = 0u64;
    let mut total_cost = 0.0f64;
    let mut total_latency = 0u64;
    let mut by_model: HashMap<String, (u64, u64, u64, f64, u64)> = HashMap::new();

    for line in content.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() < 6 {
            continue;
        }
        let model = parts[2].to_string();
        let latency = parts[3].parse::<u64>().unwrap_or(0);
        let cost = parts[4].parse::<f64>().unwrap_or(0.0);
        let ok = parts[5].trim() == "1";
        let ver = parts.get(6).map(|v| v.trim() == "1").unwrap_or(false);

        total += 1;
        if ok {
            success += 1;
        }
        if ver {
            verified += 1;
        }
        total_cost += cost;
        total_latency += latency;

        let e = by_model.entry(model).or_insert((0, 0, 0, 0.0, 0));
        e.0 += 1;
        if ok {
            e.1 += 1;
        }
        if ver {
            e.2 += 1;
        }
        e.3 += cost;
        e.4 += latency;
    }

    let success_rate = if total == 0 {
        0.0
    } else {
        success as f64 / total as f64
    };
    let avg_latency = if total == 0 {
        0.0
    } else {
        total_latency as f64 / total as f64
    };
    let verified_rate = if total == 0 {
        0.0
    } else {
        verified as f64 / total as f64
    };

    let mut model_bits = Vec::new();
    for (m, (cnt, ok, ver, cost, lat)) in by_model {
        let sr = if cnt == 0 {
            0.0
        } else {
            ok as f64 / cnt as f64
        };
        let vr = if cnt == 0 {
            0.0
        } else {
            ver as f64 / cnt as f64
        };
        let al = if cnt == 0 {
            0.0
        } else {
            lat as f64 / cnt as f64
        };
        model_bits.push(format!(
            "(:model \"{}\" :count {} :success-rate {:.4} :verified-rate {:.4} :cost-usd {:.8} :avg-latency-ms {:.2})",
            m, cnt, sr, vr, cost, al
        ));
    }
    model_bits.sort();

    Ok(format!(
        "(:total {} :success-rate {:.4} :verified-rate {:.4} :total-cost-usd {:.8} :avg-latency-ms {:.2} :models ({}))",
        total,
        success_rate,
        verified_rate,
        total_cost,
        avg_latency,
        model_bits.join(" ")
    ))
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
