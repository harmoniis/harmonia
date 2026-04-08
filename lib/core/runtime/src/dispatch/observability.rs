//! Observability component dispatch — init/status + trace fast-path.

use serde_json::json;

use harmonia_actor_protocol::extract_sexp_string;
use harmonia_observability::ObsMsg;

use super::param;

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => {
            let rc = harmonia_observability::harmonia_observability_init();
            if rc == 0 {
                let enabled = harmonia_observability::harmonia_observability_enabled();
                format!("(:ok :enabled {})", if enabled { "t" } else { "nil" })
            } else {
                "(:error \"observability init failed\")".to_string()
            }
        }
        "status" => {
            let enabled = harmonia_observability::harmonia_observability_enabled();
            let verbose = harmonia_observability::harmonia_observability_is_verbose();
            let standard = harmonia_observability::harmonia_observability_is_standard();
            let level = if verbose { "verbose" } else if standard { "standard" } else { "minimal" };
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
            let run_id = param!(sexp, ":run-id");
            let name = param!(sexp, ":name");
            let kind = param!(sexp, ":kind", "chain");
            let parent_run_id = extract_sexp_string(sexp, ":parent-run-id");
            let trace_id = extract_sexp_string(sexp, ":trace-id");
            let metadata_str = param!(sexp, ":metadata");
            let metadata_val: serde_json::Value =
                serde_json::from_str(&plist_to_json(&metadata_str)).unwrap_or(json!({}));
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
            let run_id = param!(sexp, ":run-id");
            let status = param!(sexp, ":status", "success");
            let output_str = param!(sexp, ":output");
            let outputs: serde_json::Value =
                serde_json::from_str(&plist_to_json(&output_str)).unwrap_or(json!({}));
            let _ = obs.cast(ObsMsg::SpanEnd {
                run_id,
                status,
                outputs,
            });
        }
        "trace-event" => {
            let name = param!(sexp, ":name");
            let kind = param!(sexp, ":kind", "chain");
            let metadata_str = param!(sexp, ":metadata");
            let metadata_val: serde_json::Value =
                serde_json::from_str(&plist_to_json(&metadata_str)).unwrap_or(json!({}));
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
fn plist_to_json(plist: &str) -> String {
    if plist.is_empty() {
        return "{}".to_string();
    }
    let trimmed = plist.trim().trim_start_matches('(').trim_end_matches(')');
    if trimmed.is_empty() {
        return "{}".to_string();
    }

    let tokens = tokenize_plist(trimmed);
    let pairs: String = tokens
        .chunks(2)
        .filter_map(|pair| {
            if pair.len() < 2 { return None; }
            let json_key = pair[0].trim_start_matches(':').to_lowercase();
            if json_key.is_empty() { return None; }
            let clean_val = pair[1].trim_matches('"');
            let json_val = match clean_val {
                "t" | "T" => "true".to_string(),
                "nil" | "NIL" => "false".to_string(),
                v if v.parse::<f64>().is_ok() => v.to_string(),
                v => format!("\"{}\"", v.replace('\\', "\\\\").replace('"', "\\\"")),
            };
            Some(format!("\"{}\":{}", json_key, json_val))
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{}}}", pairs)
}

/// Tokenize a plist string respecting quoted strings.
/// State machine expressed as fold over bytes.
fn tokenize_plist(s: &str) -> Vec<String> {
    #[derive(Clone, Copy)]
    enum State { Normal, InQuote, Escape }

    let (tokens, current, _) = s.as_bytes().iter().fold(
        (Vec::<String>::new(), Vec::<u8>::new(), State::Normal),
        |(mut tokens, mut current, state), &b| match state {
            State::Normal => match b {
                b' ' | b'\t' | b'\n' | b'\r' | b')' => {
                    if !current.is_empty() {
                        tokens.push(String::from_utf8_lossy(&current).into_owned());
                        current.clear();
                    }
                    (tokens, current, State::Normal)
                }
                b'"' => {
                    current.push(b);
                    (tokens, current, State::InQuote)
                }
                _ => {
                    current.push(b);
                    (tokens, current, State::Normal)
                }
            },
            State::InQuote => match b {
                b'"' => {
                    current.push(b);
                    tokens.push(String::from_utf8_lossy(&current).into_owned());
                    current.clear();
                    (tokens, current, State::Normal)
                }
                b'\\' => {
                    current.push(b);
                    (tokens, current, State::Escape)
                }
                _ => {
                    current.push(b);
                    (tokens, current, State::InQuote)
                }
            },
            State::Escape => {
                current.push(b);
                (tokens, current, State::InQuote)
            }
        },
    );
    if current.is_empty() {
        tokens
    } else {
        let mut tokens = tokens;
        tokens.push(String::from_utf8_lossy(&current).into_owned());
        tokens
    }
}
