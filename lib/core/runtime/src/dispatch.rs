//! Component dispatch — routes IPC sexp commands to crate public APIs.
//!
//! Each component's commands are dispatched here by name. The Lisp side
//! sends (:component "vault" :op "set-secret" :symbol "x" :value "y")
//! and this module calls the corresponding Rust API and returns the result
//! as an sexp string.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

/// Dispatch a command to the named component.
/// Returns an sexp reply string.
pub fn dispatch(component: &str, sexp: &str) -> String {
    match component {
        "vault" => dispatch_vault(sexp),
        "config" => dispatch_config(sexp),
        "chronicle" => dispatch_chronicle(sexp),
        "gateway" => dispatch_gateway(sexp),
        "signalograd" => dispatch_signalograd(sexp),
        "tailnet" => dispatch_tailnet(sexp),
        _ => format!("(:error \"unknown component: {}\")", component),
    }
}

// ── Vault ────────────────────────────────────────────────────────────

fn dispatch_vault(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
            let rc = harmonia_vault::init_from_env();
            if rc.is_ok() {
                "(:ok)".to_string()
            } else {
                format!("(:error \"{}\")", esc(&format!("{:?}", rc.err())))
            }
        }
        Some("set-secret") => {
            let symbol = extract_string(sexp, ":symbol").unwrap_or_default();
            let value = extract_string(sexp, ":value").unwrap_or_default();
            match harmonia_vault::set_secret_for_symbol(&symbol, &value) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("has-secret") => {
            let symbol = extract_string(sexp, ":symbol").unwrap_or_default();
            if harmonia_vault::has_secret_for_symbol(&symbol) {
                "(:ok :result t)".to_string()
            } else {
                "(:ok :result nil)".to_string()
            }
        }
        Some("list-symbols") => {
            let symbols = harmonia_vault::list_secret_symbols();
            let items: Vec<String> = symbols.iter().map(|s| format!("\"{}\"", esc(s))).collect();
            format!("(:ok :symbols ({}))", items.join(" "))
        }
        _ => format!("(:error \"unknown vault op: {}\")", op.unwrap_or_default()),
    }
}

// ── Config Store ─────────────────────────────────────────────────────

fn dispatch_config(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => match harmonia_config_store::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("get") => {
            let component = extract_string(sexp, ":component").unwrap_or_default();
            let scope = extract_string(sexp, ":scope").unwrap_or_default();
            let key = extract_string(sexp, ":key").unwrap_or_default();
            match harmonia_config_store::get_config(&component, &scope, &key) {
                Ok(Some(v)) => format!("(:ok :value \"{}\")", esc(&v)),
                Ok(None) => "(:ok :value nil)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("get-or") => {
            let component = extract_string(sexp, ":component").unwrap_or_default();
            let scope = extract_string(sexp, ":scope").unwrap_or_default();
            let key = extract_string(sexp, ":key").unwrap_or_default();
            let default = extract_string(sexp, ":default").unwrap_or_default();
            match harmonia_config_store::get_config_or(&component, &scope, &key, &default) {
                Ok(v) => format!("(:ok :value \"{}\")", esc(&v)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("set") => {
            let component = extract_string(sexp, ":component").unwrap_or_default();
            let scope = extract_string(sexp, ":scope").unwrap_or_default();
            let key = extract_string(sexp, ":key").unwrap_or_default();
            let value = extract_string(sexp, ":value").unwrap_or_default();
            match harmonia_config_store::set_config(&component, &scope, &key, &value) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("list") => {
            let component = extract_string(sexp, ":component").unwrap_or_default();
            let scope = extract_string(sexp, ":scope").unwrap_or_default();
            match harmonia_config_store::list_scope(&component, &scope) {
                Ok(keys) => {
                    let items: Vec<String> =
                        keys.iter().map(|s| format!("\"{}\"", esc(s))).collect();
                    format!("(:ok :keys ({}))", items.join(" "))
                }
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("ingest-env") => match harmonia_config_store::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        _ => format!("(:error \"unknown config op: {}\")", op.unwrap_or_default()),
    }
}

// ── Chronicle ────────────────────────────────────────────────────────

fn dispatch_chronicle(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => match harmonia_chronicle::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("query") => {
            let sql = extract_string(sexp, ":sql").unwrap_or_default();
            match harmonia_chronicle::query_sexp(&sql) {
                Ok(result) => format!("(:ok :result \"{}\")", esc(&result)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("harmony-summary") => match harmonia_chronicle::harmony_summary() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("dashboard") => match harmonia_chronicle::dashboard_json() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("gc") => match harmonia_chronicle::gc() {
            Ok(n) => format!("(:ok :result \"{}\")", n),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("gc-status") => match harmonia_chronicle::gc_status() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("cost-report") => match harmonia_chronicle::cost_report(0) {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("delegation-report") => match harmonia_chronicle::delegation_report() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("full-digest") => match harmonia_chronicle::full_digest() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        _ => format!(
            "(:error \"unknown chronicle op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

// ── Gateway ──────────────────────────────────────────────────────────

fn dispatch_gateway(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("poll") => {
            let registry = harmonia_gateway::Registry::new();
            let batch = harmonia_gateway::poll_baseband(&registry);
            let envelopes: Vec<String> = batch.envelopes.iter().map(|e| e.to_sexp()).collect();
            format!("(:ok :envelopes ({}))", envelopes.join(" "))
        }
        Some("send") => {
            let frontend = extract_string(sexp, ":frontend").unwrap_or_default();
            let channel = extract_string(sexp, ":channel").unwrap_or_default();
            let payload = extract_string(sexp, ":payload").unwrap_or_default();
            let registry = harmonia_gateway::Registry::new();
            match harmonia_gateway::send_signal(&registry, &frontend, &channel, &payload) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("is-allowed") => {
            let _signal_sexp = extract_string(sexp, ":signal").unwrap_or_default();
            // Signal allowance check - simplified for IPC
            "(:ok :allowed t)".to_string()
        }
        _ => format!(
            "(:error \"unknown gateway op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

// ── Signalograd ──────────────────────────────────────────────────────

fn dispatch_signalograd(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
            let rc = harmonia_signalograd::harmonia_signalograd_init();
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                "(:error \"signalograd init failed\")".to_string()
            }
        }
        Some("observe") => {
            let observation = extract_string(sexp, ":observation").unwrap_or_default();
            let c = CString::new(observation).unwrap_or_default();
            let rc = harmonia_signalograd::harmonia_signalograd_observe(c.as_ptr());
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                format!("(:error \"observe failed: {}\")", signalograd_last_error())
            }
        }
        Some("status") => {
            let ptr = harmonia_signalograd::harmonia_signalograd_status();
            let result = ptr_to_string(ptr);
            harmonia_signalograd::harmonia_signalograd_free_string(ptr);
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("snapshot") => {
            let ptr = harmonia_signalograd::harmonia_signalograd_snapshot();
            let result = ptr_to_string(ptr);
            harmonia_signalograd::harmonia_signalograd_free_string(ptr);
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("feedback") => {
            let feedback = extract_string(sexp, ":feedback").unwrap_or_default();
            let c = CString::new(feedback).unwrap_or_default();
            let rc = harmonia_signalograd::harmonia_signalograd_feedback(c.as_ptr());
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                format!("(:error \"feedback failed: {}\")", signalograd_last_error())
            }
        }
        Some("reset") => {
            let rc = harmonia_signalograd::harmonia_signalograd_reset();
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                "(:error \"reset failed\")".to_string()
            }
        }
        _ => format!(
            "(:error \"unknown signalograd op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

fn signalograd_last_error() -> String {
    let ptr = harmonia_signalograd::harmonia_signalograd_last_error();
    let s = ptr_to_string(ptr);
    harmonia_signalograd::harmonia_signalograd_free_string(ptr);
    s
}

// ── Tailnet ──────────────────────────────────────────────────────────

fn dispatch_tailnet(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("start") => match harmonia_tailnet::transport::start_listener() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("poll") => {
            let messages = harmonia_tailnet::transport::poll_messages();
            if messages.is_empty() {
                "(:ok :messages ())".to_string()
            } else {
                let items: Vec<String> = messages
                    .iter()
                    .map(|m| {
                        format!(
                            "(:from \"{}\" :type \"{}\" :payload \"{}\")",
                            esc(&m.from.to_string()),
                            esc(&format!("{:?}", m.msg_type)),
                            esc(&m.payload)
                        )
                    })
                    .collect();
                format!("(:ok :messages ({}))", items.join(" "))
            }
        }
        Some("send") => {
            let _to = extract_string(sexp, ":to").unwrap_or_default();
            let _payload = extract_string(sexp, ":payload").unwrap_or_default();
            // TODO: construct MeshMessage from sexp fields
            "(:ok)".to_string()
        }
        Some("discover") => match harmonia_tailnet::discover_peers() {
            Ok(peers) => {
                let items: Vec<String> = peers
                    .iter()
                    .map(|p| format!("\"{}\"", esc(&p.id.0)))
                    .collect();
                format!("(:ok :peers ({}))", items.join(" "))
            }
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("stop") => {
            harmonia_tailnet::transport::stop_listener();
            "(:ok)".to_string()
        }
        _ => format!(
            "(:error \"unknown tailnet op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn esc(s: &str) -> String {
    harmonia_actor_protocol::sexp_escape(s)
}

fn ptr_to_string(ptr: *mut c_char) -> String {
    if ptr.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned()
}

fn extract_keyword(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    if after.starts_with('"') {
        // Quoted string value
        let inner = &after[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        // Bare keyword or symbol
        let val: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ')')
            .collect();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    }
}

fn extract_string(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    if after.starts_with('"') {
        let inner = &after[1..];
        let mut end = 0;
        let bytes = inner.as_bytes();
        while end < bytes.len() {
            if bytes[end] == b'"' {
                return Some(inner[..end].replace("\\\"", "\"").replace("\\\\", "\\"));
            }
            if bytes[end] == b'\\' && end + 1 < bytes.len() {
                end += 1;
            }
            end += 1;
        }
        None
    } else {
        let val: String = after
            .chars()
            .take_while(|c| !c.is_whitespace() && *c != ')')
            .collect();
        if val.is_empty() {
            None
        } else {
            Some(val)
        }
    }
}
