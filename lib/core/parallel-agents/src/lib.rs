use harmonia_vault::{get_secret_for_symbol, init_from_env};
use std::collections::HashMap;
use std::env;
use std::ffi::{CStr, CString};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::raw::c_char;
use std::process::Command;
use std::sync::{Arc, OnceLock, RwLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const VERSION: &[u8] = b"harmonia-parallel-agents/0.1.0\0";

#[derive(Clone, Debug)]
struct Task {
    id: u64,
    prompt: String,
    model: String,
    status: String,
    response: String,
    error: String,
    latency_ms: u64,
    cost_usd: f64,
    success: bool,
    verified: bool,
    verification_source: String,
    verification_detail: String,
    created_at: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct ModelPrice {
    usd_per_1k_input: f64,
    usd_per_1k_output: f64,
}

#[derive(Default)]
struct State {
    next_id: u64,
    tasks: HashMap<u64, Task>,
    prices: HashMap<String, ModelPrice>,
}

static STATE: OnceLock<RwLock<State>> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn state() -> &'static RwLock<State> {
    STATE.get_or_init(|| RwLock::new(State { next_id: 1, ..State::default() }))
}

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn metrics_log_path() -> String {
    env::var("HARMONIA_PARALLEL_METRICS_LOG")
        .unwrap_or_else(|_| "/tmp/harmonia/parallel_agents_metrics.tsv".to_string())
}

fn append_metric_line(task: &Task) {
    let path = metrics_log_path();
    if let Some(parent) = std::path::Path::new(&path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(
            f,
            "{}\t{}\t{}\t{}\t{:.8}\t{}\t{}\t{}\t{}",
            task.created_at,
            task.id,
            task.model,
            task.latency_ms,
            task.cost_usd,
            if task.success { 1 } else { 0 },
            if task.verified { 1 } else { 0 },
            task.verification_source,
            json_escape(&task.verification_detail)
        );
    }
}

fn json_escape(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

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

    let out = Command::new("curl")
        .arg("-sS")
        .arg("--connect-timeout")
        .arg(env::var("HARMONIA_OPENROUTER_CONNECT_TIMEOUT_SECS").unwrap_or_else(|_| "10".to_string()))
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
        return Err(format!("curl failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    if let Some(e) = extract_error_message(&stdout) {
        return Err(e);
    }
    extract_content_from_response(&stdout).ok_or_else(|| format!("missing content in response: {stdout}"))
}

fn request_exa(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = env::var("HARMONIA_EXA_API_URL").unwrap_or_else(|_| "https://api.exa.ai/search".to_string());
    let payload = format!("{{\"query\":\"{}\",\"numResults\":5}}", json_escape(query));
    let out = Command::new("curl")
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
        return Err(format!("exa curl failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn request_brave(query: &str, api_key: &str) -> Result<String, String> {
    let endpoint = env::var("HARMONIA_BRAVE_API_URL")
        .unwrap_or_else(|_| "https://api.search.brave.com/res/v1/web/search".to_string());
    let out = Command::new("curl")
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
        return Err(format!("brave curl failed: {}", String::from_utf8_lossy(&out.stderr)));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

fn make_verify_query(prompt: &str, response: &str) -> String {
    let p = prompt.chars().take(120).collect::<String>();
    let r = response.chars().take(180).collect::<String>();
    format!("verify this answer against the web: prompt={p} answer={r}")
}

fn verify_with_search(prompt: &str, response: &str) -> (bool, String, String) {
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
                // Fall through to brave if available.
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
                return (false, "none".to_string(), format!("exa={e}; brave=missing-key"));
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

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_init() -> i32 {
    if let Err(e) = init_from_env() {
        set_error(e);
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_set_model_price(
    model: *const c_char,
    usd_per_1k_input: f64,
    usd_per_1k_output: f64,
) -> i32 {
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    match state().write() {
        Ok(mut st) => {
            st.prices.insert(
                model,
                ModelPrice {
                    usd_per_1k_input,
                    usd_per_1k_output,
                },
            );
            clear_error();
            0
        }
        Err(_) => {
            set_error("parallel state lock poisoned");
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_submit(
    prompt: *const c_char,
    model: *const c_char,
) -> i64 {
    let prompt = match cstr_to_string(prompt) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let model = match cstr_to_string(model) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("parallel state lock poisoned");
            return -1;
        }
    };
    let id = st.next_id;
    st.next_id += 1;
    st.tasks.insert(
        id,
        Task {
            id,
            prompt,
            model,
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
    clear_error();
    id as i64
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_run_pending(max_parallel: i32) -> i32 {
    let pending: Vec<Task> = {
        let st = match state().read() {
            Ok(v) => v,
            Err(_) => {
                set_error("parallel state lock poisoned");
                return -1;
            }
        };
        st.tasks
            .values()
            .filter(|t| t.status == "pending")
            .cloned()
            .collect()
    };

    if pending.is_empty() {
        clear_error();
        return 0;
    }

    let key = match get_secret_for_symbol("openrouter") {
        Some(v) => v,
        None => {
            set_error("missing secret: openrouter");
            return -1;
        }
    };

    let limit = if max_parallel <= 0 { 1usize } else { max_parallel as usize };
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

    let done = match results.read() {
        Ok(v) => v.clone(),
        Err(_) => {
            set_error("parallel result lock poisoned");
            return -1;
        }
    };

    let mut st = match state().write() {
        Ok(v) => v,
        Err(_) => {
            set_error("parallel state lock poisoned");
            return -1;
        }
    };
    for t in done {
        st.tasks.insert(t.id, t.clone());
        append_metric_line(&t);
    }

    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_task_result(task_id: i64) -> *mut c_char {
    let st = match state().read() {
        Ok(v) => v,
        Err(_) => {
            set_error("parallel state lock poisoned");
            return std::ptr::null_mut();
        }
    };
    match st.tasks.get(&(task_id as u64)) {
        Some(t) => {
            clear_error();
            to_c_string(format!(
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
        None => {
            set_error("task not found");
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_report() -> *mut c_char {
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
        let sr = if cnt == 0 { 0.0 } else { ok as f64 / cnt as f64 };
        let vr = if cnt == 0 { 0.0 } else { ver as f64 / cnt as f64 };
        let al = if cnt == 0 { 0.0 } else { lat as f64 / cnt as f64 };
        model_bits.push(format!(
            "(:model \"{}\" :count {} :success-rate {:.4} :verified-rate {:.4} :cost-usd {:.8} :avg-latency-ms {:.2})",
            m, cnt, sr, vr, cost, al
        ));
    }
    model_bits.sort();

    clear_error();
    to_c_string(format!(
        "(:total {} :success-rate {:.4} :verified-rate {:.4} :total-cost-usd {:.8} :avg-latency-ms {:.2} :models ({}))",
        total,
        success_rate,
        verified_rate,
        total_cost,
        avg_latency,
        model_bits.join(" ")
    ))
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_last_error() -> *mut c_char {
    let msg = last_error()
        .read()
        .map(|v| v.clone())
        .unwrap_or_else(|_| "parallel error lock poisoned".to_string());
    to_c_string(msg)
}

#[no_mangle]
pub extern "C" fn harmonia_parallel_agents_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_parallel_agents_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_non_null() {
        assert!(!harmonia_parallel_agents_version().is_null());
    }
}
