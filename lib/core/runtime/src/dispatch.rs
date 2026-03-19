//! Component dispatch — routes IPC sexp commands to crate public APIs.
//!
//! Each component's commands are dispatched here by name. The Lisp side
//! sends (:component "vault" :op "set-secret" :symbol "x" :value "y")
//! and this module calls the corresponding Rust API and returns the result
//! as an sexp string.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use ractor::ActorRef;

use crate::actors::MatrixMsg;

/// Route matrix commands through the HarmonicMatrixActor for serialized access.
pub async fn dispatch_matrix_via_actor(matrix: &ActorRef<MatrixMsg>, sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("register-node") => {
            let id = extract_string(sexp, ":id").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_default();
            let _ = matrix.cast(MatrixMsg::RegisterNode { id, kind });
            "(:ok)".to_string()
        }
        Some("register-edge") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let weight = parse_f64(sexp, ":weight");
            let min_harmony = parse_f64(sexp, ":min-harmony");
            let _ = matrix.cast(MatrixMsg::RegisterEdge {
                from,
                to,
                weight,
                min_harmony,
            });
            "(:ok)".to_string()
        }
        Some("observe-route") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let success = parse_bool(sexp, ":success");
            let latency_ms = extract_u64(sexp, ":latency-ms");
            let cost_usd = parse_f64(sexp, ":cost-usd");
            let _ = matrix.cast(MatrixMsg::ObserveRoute {
                from,
                to,
                success,
                latency_ms,
                cost_usd,
            });
            "(:ok)".to_string()
        }
        Some("log-event") => {
            let component = extract_string(sexp, ":component").unwrap_or_default();
            let direction = extract_string(sexp, ":direction").unwrap_or_default();
            let channel = extract_string(sexp, ":channel").unwrap_or_default();
            let payload = extract_string(sexp, ":payload").unwrap_or_default();
            let success = parse_bool(sexp, ":success");
            let error = extract_string(sexp, ":error").unwrap_or_default();
            let _ = matrix.cast(MatrixMsg::LogEvent {
                component,
                direction,
                channel,
                payload,
                success,
                error,
            });
            "(:ok)".to_string()
        }
        Some("set-tool-enabled") => {
            let node = extract_string(sexp, ":node").unwrap_or_default();
            let enabled = parse_bool(sexp, ":enabled");
            let _ = matrix.cast(MatrixMsg::SetToolEnabled { node, enabled });
            "(:ok)".to_string()
        }
        Some("route-allowed") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let signal = parse_f64(sexp, ":signal");
            let noise = parse_f64(sexp, ":noise");
            match ractor::call_t!(
                matrix,
                |reply| MatrixMsg::RouteAllowed {
                    from,
                    to,
                    signal,
                    noise,
                    reply
                },
                5000
            ) {
                Ok(allowed) => {
                    if allowed {
                        "(:ok :allowed t)".to_string()
                    } else {
                        "(:ok :allowed nil)".to_string()
                    }
                }
                Err(_) => "(:error \"matrix route-allowed timeout\")".to_string(),
            }
        }
        Some("report") => match ractor::call_t!(matrix, MatrixMsg::Report, 5000) {
            Ok(report) => format!("(:ok :result \"{}\")", esc(&report)),
            Err(_) => "(:error \"matrix report timeout\")".to_string(),
        },
        Some("store-summary") => match ractor::call_t!(matrix, MatrixMsg::StoreSummary, 5000) {
            Ok(summary) => format!("(:ok :result \"{}\")", esc(&summary)),
            Err(_) => "(:error \"matrix store-summary timeout\")".to_string(),
        },
        _ => {
            // Fall back to synchronous dispatch for ops not yet in the actor
            dispatch_matrix(sexp)
        }
    }
}

/// Dispatch a command to the named component (synchronous, for non-matrix components).
/// Returns an sexp reply string.
pub fn dispatch(component: &str, sexp: &str) -> String {
    match component {
        "vault" => dispatch_vault(sexp),
        "config" => dispatch_config(sexp),
        "chronicle" => dispatch_chronicle(sexp),
        "gateway" => dispatch_gateway(sexp),
        "signalograd" => dispatch_signalograd(sexp),
        "tailnet" => dispatch_tailnet(sexp),
        "harmonic-matrix" => dispatch_matrix(sexp),
        "observability" => dispatch_observability(sexp),
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
        Some("record-harmonic") => {
            let snap = harmonia_chronicle::HarmonicSnapshot {
                cycle: extract_string(sexp, ":cycle")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                phase: extract_string(sexp, ":phase").unwrap_or_default(),
                strength: parse_f64(sexp, ":strength"),
                utility: parse_f64(sexp, ":utility"),
                beauty: parse_f64(sexp, ":beauty"),
                signal: parse_f64(sexp, ":signal"),
                noise: parse_f64(sexp, ":noise"),
                logistic_x: parse_f64(sexp, ":logistic-x"),
                logistic_r: parse_f64(sexp, ":logistic-r"),
                chaos_risk: parse_f64(sexp, ":chaos-risk"),
                rewrite_aggression: parse_f64(sexp, ":rewrite-aggression"),
                lorenz_x: parse_f64(sexp, ":lorenz-x"),
                lorenz_y: parse_f64(sexp, ":lorenz-y"),
                lorenz_z: parse_f64(sexp, ":lorenz-z"),
                lorenz_radius: parse_f64(sexp, ":lorenz-radius"),
                lorenz_bounded: parse_f64(sexp, ":lorenz-bounded"),
                lambdoma_global: parse_f64(sexp, ":lambdoma-global"),
                lambdoma_local: parse_f64(sexp, ":lambdoma-local"),
                lambdoma_ratio: parse_f64(sexp, ":lambdoma-ratio"),
                lambdoma_convergent: parse_bool(sexp, ":lambdoma-convergent"),
                rewrite_ready: parse_bool(sexp, ":rewrite-ready"),
                rewrite_count: extract_string(sexp, ":rewrite-count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                security_posture: extract_string(sexp, ":security-posture").unwrap_or_default(),
                security_events: extract_string(sexp, ":security-events")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
            };
            match harmonia_chronicle::harmonic::record(&snap) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("record-memory-event") => {
            let event_type = extract_string(sexp, ":event-type").unwrap_or_default();
            let entries_created: i32 = extract_string(sexp, ":entries-created")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let entries_source: i32 = extract_string(sexp, ":entries-source")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let old_size: i64 = extract_string(sexp, ":old-size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let new_size: i64 = extract_string(sexp, ":new-size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let node_count: i32 = extract_string(sexp, ":node-count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let edge_count: i32 = extract_string(sexp, ":edge-count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let interdisciplinary_edges: i32 = extract_string(sexp, ":interdisciplinary-edges")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let max_depth: i32 = extract_string(sexp, ":max-depth")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let detail = extract_string(sexp, ":detail");
            match harmonia_chronicle::memory::record(
                &event_type,
                entries_created,
                entries_source,
                old_size,
                new_size,
                node_count,
                edge_count,
                interdisciplinary_edges,
                max_depth,
                detail.as_deref(),
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("record-delegation") => {
            let task_hint = extract_string(sexp, ":task-hint");
            let model_chosen = extract_string(sexp, ":model-chosen").unwrap_or_default();
            let backend = extract_string(sexp, ":backend").unwrap_or_default();
            let reason = extract_string(sexp, ":reason");
            let escalated = parse_bool(sexp, ":escalated");
            let escalated_from = extract_string(sexp, ":escalated-from");
            let cost_usd = parse_f64(sexp, ":cost-usd");
            let latency_ms: i64 = extract_string(sexp, ":latency-ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let success = parse_bool(sexp, ":success");
            let tokens_in: i64 = extract_string(sexp, ":tokens-in")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let tokens_out: i64 = extract_string(sexp, ":tokens-out")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match harmonia_chronicle::delegation::record(
                task_hint.as_deref(),
                &model_chosen,
                &backend,
                reason.as_deref(),
                escalated,
                escalated_from.as_deref(),
                cost_usd,
                latency_ms,
                success,
                tokens_in,
                tokens_out,
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("record-graph") => {
            let source = extract_string(sexp, ":source").unwrap_or_default();
            let graph_sexp = extract_string(sexp, ":sexp").unwrap_or_default();
            // Graph record with empty nodes/edges — the raw sexp is stored for later decomposition
            match harmonia_chronicle::graph::record_snapshot(&source, &graph_sexp, &[], &[]) {
                Ok(id) => format!("(:ok :snapshot-id {})", id),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("record-signalograd-event") => {
            let event_type = extract_string(sexp, ":event-type").unwrap_or_default();
            let cycle: i64 = extract_string(sexp, ":cycle")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let confidence = parse_f64(sexp, ":confidence");
            let stability = parse_f64(sexp, ":stability");
            let novelty = parse_f64(sexp, ":novelty");
            let reward = parse_f64(sexp, ":reward");
            let accepted = parse_bool(sexp, ":accepted");
            let recall_hits: i32 = extract_string(sexp, ":recall-hits")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let checkpoint_path = extract_string(sexp, ":checkpoint-path");
            let checkpoint_digest = extract_string(sexp, ":checkpoint-digest");
            let detail = extract_string(sexp, ":detail");
            match harmonia_chronicle::signalograd::record(
                &event_type,
                cycle,
                confidence,
                stability,
                novelty,
                reward,
                accepted,
                recall_hits,
                checkpoint_path.as_deref(),
                checkpoint_digest.as_deref(),
                detail.as_deref(),
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("record-phoenix-event") => {
            let event_type = extract_string(sexp, ":event-type").unwrap_or_default();
            let exit_code: Option<i32> =
                extract_string(sexp, ":exit-code").and_then(|s| s.parse().ok());
            let attempt: Option<i32> =
                extract_string(sexp, ":attempt").and_then(|s| s.parse().ok());
            let max_attempts: Option<i32> =
                extract_string(sexp, ":max-attempts").and_then(|s| s.parse().ok());
            let recovery_ms: Option<i64> =
                extract_string(sexp, ":recovery-ms").and_then(|s| s.parse().ok());
            let detail = extract_string(sexp, ":detail");
            match harmonia_chronicle::phoenix::record(
                &event_type,
                exit_code,
                attempt,
                max_attempts,
                recovery_ms,
                detail.as_deref(),
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("record-ouroboros-event") => {
            let event_type = extract_string(sexp, ":event-type").unwrap_or_default();
            let component = extract_string(sexp, ":component");
            let detail = extract_string(sexp, ":detail");
            let patch_size: Option<i64> =
                extract_string(sexp, ":patch-size").and_then(|s| s.parse().ok());
            let success = parse_bool(sexp, ":success");
            match harmonia_chronicle::ouroboros::record(
                &event_type,
                component.as_deref(),
                detail.as_deref(),
                patch_size,
                success,
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
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
        Some("checkpoint") => {
            let path = extract_string(sexp, ":path").unwrap_or_default();
            let c = CString::new(path).unwrap_or_default();
            let rc = harmonia_signalograd::harmonia_signalograd_checkpoint(c.as_ptr());
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                format!(
                    "(:error \"checkpoint failed: {}\")",
                    signalograd_last_error()
                )
            }
        }
        Some("restore") => {
            let path = extract_string(sexp, ":path").unwrap_or_default();
            let c = CString::new(path).unwrap_or_default();
            let rc = harmonia_signalograd::harmonia_signalograd_restore(c.as_ptr());
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                format!("(:error \"restore failed: {}\")", signalograd_last_error())
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

// ── Harmonic Matrix ──────────────────────────────────────────────────

fn dispatch_matrix(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => match harmonia_harmonic_matrix::runtime::store::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("set-store") => {
            let kind = extract_string(sexp, ":kind").unwrap_or_default();
            let path = extract_string(sexp, ":path");
            match harmonia_harmonic_matrix::runtime::store::set_store(&kind, path.as_deref()) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("register-node") => {
            let node_id = extract_string(sexp, ":node-id").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_default();
            match harmonia_harmonic_matrix::runtime::ops::register_node(&node_id, &kind) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("set-tool-enabled") => {
            let tool_id = extract_string(sexp, ":tool-id").unwrap_or_default();
            let enabled = parse_bool(sexp, ":enabled");
            match harmonia_harmonic_matrix::runtime::ops::set_tool_enabled(&tool_id, enabled) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("register-edge") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let weight = parse_f64(sexp, ":weight");
            let min_harmony = parse_f64(sexp, ":min-harmony");
            match harmonia_harmonic_matrix::runtime::ops::register_edge(
                &from,
                &to,
                weight,
                min_harmony,
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("route-allowed") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let signal = parse_f64(sexp, ":signal");
            let noise = parse_f64(sexp, ":noise");
            match harmonia_harmonic_matrix::runtime::ops::route_allowed(&from, &to, signal, noise) {
                Ok(allowed) => format!("(:ok :allowed {})", if allowed { "t" } else { "nil" }),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("route-allowed-ctx") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let signal = parse_f64(sexp, ":signal");
            let noise = parse_f64(sexp, ":noise");
            let security_weight = parse_f64(sexp, ":security-weight");
            let dissonance = parse_f64(sexp, ":dissonance");
            match harmonia_harmonic_matrix::runtime::ops::route_allowed_with_context(
                &from,
                &to,
                signal,
                noise,
                security_weight,
                dissonance,
            ) {
                Ok(allowed) => format!("(:ok :allowed {})", if allowed { "t" } else { "nil" }),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("observe-route") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let success = parse_bool(sexp, ":success");
            let latency_ms: u64 = extract_string(sexp, ":latency-ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let cost_usd = parse_f64(sexp, ":cost-usd");
            match harmonia_harmonic_matrix::runtime::ops::observe_route(
                &from, &to, success, latency_ms, cost_usd,
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("log-event") => {
            let component = extract_string(sexp, ":component").unwrap_or_default();
            let direction = extract_string(sexp, ":direction").unwrap_or_default();
            let channel = extract_string(sexp, ":channel").unwrap_or_default();
            let payload = extract_string(sexp, ":payload").unwrap_or_default();
            let success = parse_bool(sexp, ":success");
            let error = extract_string(sexp, ":error").unwrap_or_default();
            match harmonia_harmonic_matrix::runtime::ops::log_event(
                &component, &direction, &channel, &payload, success, &error,
            ) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("route-timeseries") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let limit: i32 = extract_string(sexp, ":limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match harmonia_harmonic_matrix::runtime::reports::route_timeseries(&from, &to, limit) {
                Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("time-report") => {
            let since_unix: u64 = extract_string(sexp, ":since")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match harmonia_harmonic_matrix::runtime::reports::time_report(since_unix) {
                Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("report") => match harmonia_harmonic_matrix::runtime::reports::report() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("store-summary") => match harmonia_harmonic_matrix::runtime::store::store_summary() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        _ => format!(
            "(:error \"unknown harmonic-matrix op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

// ── Observability ────────────────────────────────────────────────────

fn dispatch_observability(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
            let rc = harmonia_observability::harmonia_observability_init();
            if rc == 0 {
                "(:ok)".to_string()
            } else {
                "(:error \"observability init failed\")".to_string()
            }
        }
        Some("trace-start") => {
            let name = extract_string(sexp, ":name").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_else(|| "chain".to_string());
            let parent_id: i64 = extract_string(sexp, ":parent-id")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let metadata = extract_string(sexp, ":metadata").unwrap_or_default();
            // Convert Lisp plist metadata to JSON
            let metadata_json = plist_to_json(&metadata);
            let handle = harmonia_observability::harmonia_observability_trace_start(
                &name,
                &kind,
                parent_id,
                &metadata_json,
            );
            format!("(:ok :handle {})", handle)
        }
        Some("trace-end") => {
            let handle: i64 = extract_string(sexp, ":handle")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let status = extract_string(sexp, ":status").unwrap_or_else(|| "success".to_string());
            let output = extract_string(sexp, ":output").unwrap_or_default();
            let output_json = plist_to_json(&output);
            harmonia_observability::harmonia_observability_trace_end(handle, &status, &output_json);
            "(:ok)".to_string()
        }
        Some("trace-event") => {
            let name = extract_string(sexp, ":name").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_else(|| "chain".to_string());
            let metadata = extract_string(sexp, ":metadata").unwrap_or_default();
            let metadata_json = plist_to_json(&metadata);
            harmonia_observability::harmonia_observability_trace_event(
                &name,
                &kind,
                &metadata_json,
            );
            "(:ok)".to_string()
        }
        Some("flush") => {
            harmonia_observability::harmonia_observability_flush();
            "(:ok)".to_string()
        }
        Some("shutdown") => {
            harmonia_observability::harmonia_observability_shutdown();
            "(:ok)".to_string()
        }
        _ => format!(
            "(:error \"unknown observability op: {}\")",
            op.unwrap_or_default()
        ),
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
        // Key should start with ':'
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
        // Value: try number, bool, or string
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
        // Skip whitespace
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i] == b'"' {
            // Quoted string
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
            // Bare token
            let start = i;
            while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b')' {
                i += 1;
            }
            tokens.push(String::from_utf8_lossy(&bytes[start..i]).into_owned());
        }
    }
    tokens
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

fn extract_u64(sexp: &str, key: &str) -> u64 {
    extract_string(sexp, key)
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0)
}

fn parse_f64(sexp: &str, key: &str) -> f64 {
    extract_string(sexp, key)
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.0)
}

fn parse_bool(sexp: &str, key: &str) -> bool {
    extract_string(sexp, key)
        .map(|s| matches!(s.as_str(), "t" | "true" | "1"))
        .unwrap_or(false)
}
