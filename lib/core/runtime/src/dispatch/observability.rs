//! Observability component dispatch — init/status + trace fast-path.

use serde_json::json;

use harmonia_actor_protocol::extract_sexp_string;
use harmonia_observability::ObsMsg;

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => {
            let rc = harmonia_observability::harmonia_observability_init();
            if rc == 0 {
                let enabled = harmonia_observability::harmonia_observability_enabled();
                if enabled {
                    "(:ok :enabled t)".to_string()
                } else {
                    "(:ok :enabled nil)".to_string()
                }
            } else {
                "(:error \"observability init failed\")".to_string()
            }
        }
        "status" => {
            let enabled = harmonia_observability::harmonia_observability_enabled();
            let verbose = harmonia_observability::harmonia_observability_is_verbose();
            let standard = harmonia_observability::harmonia_observability_is_standard();
            let level = if verbose {
                "verbose"
            } else if standard {
                "standard"
            } else {
                "minimal"
            };
            let sample_rate = harmonia_observability::get_config()
                .map(|c| c.sample_rate)
                .unwrap_or(0.1);
            format!(
                "(:ok :enabled {} :level \"{}\" :sample-rate {})",
                if enabled { "t" } else { "nil" },
                level,
                sample_rate
            )
        }
        // Trace ops — cast to obs actor (fire-and-forget)
        "trace-start" | "trace-end" | "trace-event" => {
            dispatch_obs_trace(&op, sexp);
            "(:ok)".to_string()
        }
        "flush" => {
            harmonia_observability::harmonia_observability_flush();
            "(:ok)".to_string()
        }
        "shutdown" => {
            harmonia_observability::harmonia_observability_shutdown();
            "(:ok)".to_string()
        }
        _ => format!("(:error \"unknown observability op: {}\")", op),
    }
}

/// Fast-path dispatch for observability trace ops.
/// Casts to the ObservabilityActor and returns immediately.
/// Called from ipc.rs (fire-and-forget) and dispatch (fallback).
pub fn dispatch_obs_trace(op: &str, sexp: &str) {
    let obs = match harmonia_observability::get_obs_actor() {
        Some(o) => o,
        None => return,
    };
    match op {
        "trace-start" => {
            let run_id = extract_sexp_string(sexp, ":run-id").unwrap_or_default();
            let name = extract_sexp_string(sexp, ":name").unwrap_or_default();
            let kind = extract_sexp_string(sexp, ":kind").unwrap_or_else(|| "chain".to_string());
            let parent_run_id = extract_sexp_string(sexp, ":parent-run-id");
            let trace_id = extract_sexp_string(sexp, ":trace-id");
            let metadata_str = extract_sexp_string(sexp, ":metadata").unwrap_or_default();
            let metadata_json = plist_to_json(&metadata_str);
            let metadata_val: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(json!({}));
            let _ = obs.cast(ObsMsg::SpanStart {
                run_id,
                parent_run_id,
                trace_id,
                name,
                run_type: kind,
                metadata: metadata_val,
            });
        }
        "trace-end" => {
            let run_id = extract_sexp_string(sexp, ":run-id").unwrap_or_default();
            let status = extract_sexp_string(sexp, ":status").unwrap_or_else(|| "success".to_string());
            let output_str = extract_sexp_string(sexp, ":output").unwrap_or_default();
            let output_json = plist_to_json(&output_str);
            let outputs: serde_json::Value =
                serde_json::from_str(&output_json).unwrap_or(json!({}));
            let _ = obs.cast(ObsMsg::SpanEnd {
                run_id,
                status,
                outputs,
            });
        }
        "trace-event" => {
            let name = extract_sexp_string(sexp, ":name").unwrap_or_default();
            let kind = extract_sexp_string(sexp, ":kind").unwrap_or_else(|| "chain".to_string());
            let metadata_str = extract_sexp_string(sexp, ":metadata").unwrap_or_default();
            let metadata_json = plist_to_json(&metadata_str);
            let metadata_val: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(json!({}));
            let parent_run_id = extract_sexp_string(sexp, ":parent-run-id");
            let trace_id = extract_sexp_string(sexp, ":trace-id");
            let _ = obs.cast(ObsMsg::Event {
                name,
                run_type: kind,
                metadata: metadata_val,
                parent_run_id,
                trace_id,
            });
        }
        _ => {}
    }
}

/// Best-effort conversion of a Lisp plist string to JSON.
/// Input: "(:key1 val1 :key2 \"val2\")" or "(KEY1 VAL1 KEY2 VAL2)"
/// Handles the common metadata patterns from Lisp trace calls.
fn plist_to_json(plist: &str) -> String {
    if plist.is_empty() {
        return "{}".to_string();
    }
    let trimmed = plist.trim().trim_start_matches('(').trim_end_matches(')');
    if trimmed.is_empty() {
        return "{}".to_string();
    }

    let mut result = String::from("{");
    let mut first = true;
    let tokens = tokenize_plist(trimmed);
    let mut i = 0;
    while i + 1 < tokens.len() {
        let key = &tokens[i];
        let val = &tokens[i + 1];
        let json_key = key.trim_start_matches(':').to_lowercase();
        if json_key.is_empty() {
            i += 2;
            continue;
        }
        if !first {
            result.push(',');
        }
        first = false;
        result.push('"');
        result.push_str(&json_key);
        result.push_str("\":");
        let clean_val = val.trim_matches('"');
        if clean_val == "t" || clean_val == "T" {
            result.push_str("true");
        } else if clean_val == "nil" || clean_val == "NIL" {
            result.push_str("false");
        } else if clean_val.parse::<f64>().is_ok() {
            result.push_str(clean_val);
        } else {
            result.push('"');
            result.push_str(&clean_val.replace('\\', "\\\\").replace('"', "\\\""));
            result.push('"');
        }
        i += 2;
    }
    result.push('}');
    result
}

/// Tokenize a plist string respecting quoted strings.
fn tokenize_plist(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            let mut end = i + 1;
            while end < bytes.len() {
                if bytes[end] == b'"' {
                    break;
                }
                if bytes[end] == b'\\' && end + 1 < bytes.len() {
                    end += 1;
                }
                end += 1;
            }
            tokens.push(String::from_utf8_lossy(&bytes[i..=end.min(bytes.len() - 1)]).into_owned());
            i = end + 1;
        } else {
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b')' {
                i += 1;
            }
            tokens.push(String::from_utf8_lossy(&bytes[start..i]).into_owned());
        }
    }
    tokens
}
