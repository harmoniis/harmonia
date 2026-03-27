//! Component dispatch — routes IPC sexp commands to crate public APIs.
//!
//! Each component's commands are dispatched here by name. The Lisp side
//! sends (:component "vault" :op "set-secret" :symbol "x" :value "y")
//! and this module calls the corresponding Rust API and returns the result
//! as an sexp string.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use ractor::ActorRef;
use serde_json::json;

use harmonia_observability::{ObsMsg, Traceable};

use crate::actors::MatrixMsg;

/// Convert a string to CString for FFI, returning error sexp on null bytes.
fn to_cstring(s: &str) -> Result<CString, String> {
    CString::new(s).map_err(|_| format!("(:error \"string contains null byte\")"))
}

/// RAII guard for strings allocated by C FFI. Automatically freed on drop.
struct FfiString(*mut c_char);
impl FfiString {
    fn as_str(&self) -> &str {
        if self.0.is_null() {
            return "";
        }
        unsafe { CStr::from_ptr(self.0) }.to_str().unwrap_or("")
    }
}
impl Drop for FfiString {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // Each component has its own free function, but the pattern is always CString::from_raw
            unsafe {
                drop(CString::from_raw(self.0));
            }
        }
    }
}

/// Route matrix commands through the HarmonicMatrixActor for serialized access.
/// Observability traces matrix operations via the obs actor (no statics, no mutex).
pub async fn dispatch_matrix_via_actor(
    matrix: &ActorRef<MatrixMsg>,
    obs: &Option<ActorRef<ObsMsg>>,
    sexp: &str,
) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("register-node") => {
            let id = extract_string(sexp, ":id").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_default();
            if harmonia_observability::harmonia_observability_is_verbose() {
                obs.trace_event(
                    "matrix-topology",
                    "chain",
                    json!({"op": "register-node", "node": id.clone(), "kind": kind.clone()}),
                );
            }
            let _ = matrix.cast(MatrixMsg::RegisterNode { id, kind });
            "(:ok)".to_string()
        }
        Some("register-edge") => {
            let from = extract_string(sexp, ":from").unwrap_or_default();
            let to = extract_string(sexp, ":to").unwrap_or_default();
            let weight = parse_f64(sexp, ":weight");
            let min_harmony = parse_f64(sexp, ":min-harmony");
            if harmonia_observability::harmonia_observability_is_verbose() {
                obs.trace_event("matrix-topology", "chain", json!({"op": "register-edge", "from": from.clone(), "to": to.clone(), "weight": weight, "min_harmony": min_harmony}));
            }
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
            if harmonia_observability::harmonia_observability_is_standard() {
                obs.trace_event("matrix-route-observed", "chain", json!({"from": from.clone(), "to": to.clone(), "success": success, "latency_ms": latency_ms, "cost_usd": cost_usd}));
            }
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
            if harmonia_observability::harmonia_observability_is_verbose() {
                obs.trace_event("matrix-event", "chain", json!({"component": component.clone(), "direction": direction.clone(), "channel": channel.clone(), "success": success, "error": error.clone()}));
            }
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
            if harmonia_observability::harmonia_observability_is_verbose() {
                obs.trace_event(
                    "matrix-topology",
                    "chain",
                    json!({"op": "set-tool-enabled", "node": node.clone(), "enabled": enabled}),
                );
            }
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
                    from: from.clone(),
                    to: to.clone(),
                    signal,
                    noise,
                    reply
                },
                5000
            ) {
                Ok(allowed) => {
                    if harmonia_observability::harmonia_observability_is_standard() {
                        obs.trace_event("matrix-route-decision", "chain", json!({"from": from.clone(), "to": to.clone(), "signal": signal, "noise": noise, "snr": if noise > 0.0 { signal / noise } else { signal }, "allowed": allowed}));
                    }
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
        "signalograd" => "(:error \"signalograd dispatch requires actor-owned state\")".into(),
        "tailnet" => dispatch_tailnet(sexp),
        "harmonic-matrix" => dispatch_matrix(sexp),
        "observability" => dispatch_observability(sexp),
        "provider-router" => dispatch_provider_router(sexp),
        "parallel" => dispatch_parallel(sexp),
        "router" => "(:ok :result \"router dispatch via actor\")".to_string(),
        "memory-field" => "(:error \"memory-field dispatch requires actor-owned state\")".into(),
        _ => format!("(:error \"unknown component: {}\")", component),
    }
}

// ── Provider Router ──────────────────────────────────────────────────

fn dispatch_provider_router(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("complete") => {
            let prompt = extract_string(sexp, ":prompt").unwrap_or_default();
            let model = extract_string(sexp, ":model").unwrap_or_default();
            let prompt_c = match to_cstring(prompt.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let model_c = match to_cstring(model.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let model_ptr = if model.is_empty() {
                std::ptr::null()
            } else {
                model_c.as_ptr()
            };
            if harmonia_observability::harmonia_observability_is_standard() {
                let obs_ref = harmonia_observability::get_obs_actor().cloned();
                obs_ref.trace_event(
                    "provider-route",
                    "chain",
                    json!({"model": model.clone(), "op": "complete"}),
                );
            }
            let result_ptr = harmonia_provider_router::harmonia_provider_router_complete(
                prompt_c.as_ptr(),
                model_ptr,
            );
            let ffi_result = FfiString(result_ptr);
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("complete-for-task") => {
            let prompt = extract_string(sexp, ":prompt").unwrap_or_default();
            let task = extract_string(sexp, ":task").unwrap_or_default();
            let prompt_c = match to_cstring(prompt.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let task_c = match to_cstring(task.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let result_ptr = harmonia_provider_router::harmonia_provider_router_complete_for_task(
                prompt_c.as_ptr(),
                task_c.as_ptr(),
            );
            let ffi_result = FfiString(result_ptr);
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("healthcheck") => {
            let rc = harmonia_provider_router::harmonia_provider_router_healthcheck();
            format!("(:ok :healthy {})", if rc == 1 { "t" } else { "nil" })
        }
        Some("list-models") => {
            let ffi_result =
                FfiString(harmonia_provider_router::harmonia_provider_router_list_models());
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("select-model") => {
            let task = extract_string(sexp, ":task").unwrap_or_default();
            let task_c = match to_cstring(task.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let ffi_result = FfiString(
                harmonia_provider_router::harmonia_provider_router_select_model(task_c.as_ptr()),
            );
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("list-backends") => {
            let ffi_result =
                FfiString(harmonia_provider_router::harmonia_provider_router_list_backends());
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("backend-status") => {
            let name = extract_string(sexp, ":name").unwrap_or_default();
            let name_c = match to_cstring(name.as_str()) {
                Ok(c) => c,
                Err(e) => return e,
            };
            let ffi_result = FfiString(
                harmonia_provider_router::harmonia_provider_router_backend_status(name_c.as_ptr()),
            );
            let result = ffi_result.as_str().to_string();
            format!("(:ok :result \"{}\")", esc(&result))
        }
        _ => format!(
            "(:error \"unknown provider-router op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

// ── Parallel Agents ──────────────────────────────────────────────────

fn dispatch_parallel(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
            // Already initialized via init_all
            "(:ok)".to_string()
        }
        Some("submit") => {
            let prompt = extract_string(sexp, ":prompt").unwrap_or_default();
            let model = extract_string(sexp, ":model").unwrap_or_default();
            match harmonia_parallel_agents::engine::submit(&prompt, &model) {
                Ok(task_id) => format!("(:ok :task-id {})", task_id),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("run-pending") => {
            let max = extract_string(sexp, ":max-parallel")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            match harmonia_parallel_agents::engine::run_pending(max) {
                Ok(()) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("run-pending-async") => {
            let max = extract_string(sexp, ":max-parallel")
                .and_then(|s| s.parse::<i32>().ok())
                .unwrap_or(3);
            match harmonia_parallel_agents::engine::run_pending_async(max) {
                Ok(assignments) => {
                    let items: Vec<String> = assignments
                        .iter()
                        .map(|(tid, aid, model)| {
                            format!(
                                "(:task-id {} :actor-id {} :model \"{}\")",
                                tid,
                                aid,
                                esc(model)
                            )
                        })
                        .collect();
                    format!("(:ok :assignments ({}))", items.join(" "))
                }
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("task-result") => {
            let id = extract_string(sexp, ":task-id")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            match harmonia_parallel_agents::engine::task_result(id) {
                Ok(result) => format!("(:ok :result \"{}\")", esc(&result)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("report") => match harmonia_parallel_agents::engine::report() {
            Ok(r) => format!("(:ok :result \"{}\")", esc(&r)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        Some("set-model-price") => {
            let model = extract_string(sexp, ":model").unwrap_or_default();
            let in_price = parse_f64(sexp, ":in-price");
            let out_price = parse_f64(sexp, ":out-price");
            let _ = harmonia_parallel_agents::engine::set_model_price(&model, in_price, out_price);
            "(:ok)".to_string()
        }
        _ => format!(
            "(:error \"unknown parallel op: {}\")",
            op.unwrap_or_default()
        ),
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
                field_basin: extract_string(sexp, ":field-basin").unwrap_or_else(|| "thomas-0".into()),
                field_coercive_energy: parse_f64(sexp, ":field-coercive-energy"),
                field_dwell_ticks: extract_string(sexp, ":field-dwell-ticks")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                field_threshold: {
                    let t = parse_f64(sexp, ":field-threshold");
                    if t < 0.01 { 0.35 } else { t }
                },
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
        // ── Persistent memory entries ──────────────────────────
        Some("persist-entry") => {
            let id = extract_string(sexp, ":id").unwrap_or_default();
            let ts: i64 = extract_string(sexp, ":ts").and_then(|s| s.parse().ok()).unwrap_or(0);
            let content = extract_string(sexp, ":content").unwrap_or_default();
            let tags = extract_string(sexp, ":tags").unwrap_or_default();
            let source_ids = extract_string(sexp, ":source-ids").unwrap_or_default();
            match harmonia_chronicle::memory::persist_entry(&id, ts, &content, &tags, &source_ids) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("load-all-entries") => {
            match harmonia_chronicle::memory::load_all_entries() {
                Ok(result) => result,
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("entry-count") => {
            match harmonia_chronicle::memory::entry_count() {
                Ok(count) => format!("(:ok :count {})", count),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("update-access") => {
            let id = extract_string(sexp, ":id").unwrap_or_default();
            match harmonia_chronicle::memory::update_access(&id) {
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
            let envelopes = poll_all_frontends();
            format!("(:ok :envelopes ({}))", envelopes.join(" "))
        }
        Some("send") => {
            let frontend = extract_string(sexp, ":frontend").unwrap_or_default();
            let channel = extract_string(sexp, ":channel").unwrap_or_default();
            let payload = extract_string(sexp, ":payload").unwrap_or_default();
            let result = send_to_frontend(&frontend, &channel, &payload);
            if harmonia_observability::harmonia_observability_is_standard() {
                let obs_ref = harmonia_observability::get_obs_actor().cloned();
                obs_ref.trace_event(
                    "gateway-send",
                    "tool",
                    json!({"frontend": frontend, "channel": channel, "success": result.is_ok()}),
                );
            }
            match result {
                Ok(()) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        Some("is-allowed") => "(:ok :allowed t)".to_string(),
        _ => format!(
            "(:error \"unknown gateway op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

/// Poll ALL initialized frontends and return sexp envelopes.
fn poll_all_frontends() -> Vec<String> {
    let mut envelopes = Vec::new();

    // TUI — local trusted session
    for (address, payload) in harmonia_tui::terminal::poll() {
        envelopes.push(make_envelope("tui", &address, &payload, "owner"));
    }

    // Messaging frontends — each gracefully returns empty if not initialized
    poll_frontend(
        &mut envelopes,
        "telegram",
        harmonia_telegram::bot::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "slack",
        harmonia_slack::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "discord",
        harmonia_discord::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "signal",
        harmonia_signal::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "mattermost",
        harmonia_mattermost::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "whatsapp",
        harmonia_whatsapp::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "nostr",
        harmonia_nostr::client::poll(),
        "authenticated",
    );
    poll_frontend(
        &mut envelopes,
        "email",
        harmonia_email_client::client::poll(),
        "authenticated",
    );

    #[cfg(target_os = "macos")]
    poll_frontend(
        &mut envelopes,
        "imessage",
        harmonia_imessage::client::poll(),
        "authenticated",
    );

    poll_frontend(
        &mut envelopes,
        "tailscale",
        harmonia_tailscale_frontend::bridge::poll(),
        "authenticated",
    );

    envelopes
}

fn poll_frontend(
    envelopes: &mut Vec<String>,
    kind: &str,
    result: Result<Vec<(String, String, Option<String>)>, String>,
    label: &str,
) {
    for (address, payload, _metadata) in result.unwrap_or_default() {
        envelopes.push(make_envelope(kind, &address, &payload, label));
    }
}

fn make_envelope(kind: &str, address: &str, payload: &str, label: &str) -> String {
    format!(
        "(:channel (:kind \"{}\" :address \"{}\") :body (:text \"{}\") :peer (:device-id \"{}\") :security (:label :{}) :capabilities (:text t))",
        esc(kind),
        esc(address),
        esc(payload),
        esc(address),
        label
    )
}

/// Route an outbound message to the correct frontend.
fn send_to_frontend(frontend: &str, channel: &str, payload: &str) -> Result<(), String> {
    match frontend {
        "tui" => {
            harmonia_tui::terminal::send(channel, payload);
            Ok(())
        }
        "telegram" => harmonia_telegram::bot::send(channel, payload),
        "slack" => harmonia_slack::client::send(channel, payload),
        "discord" => harmonia_discord::client::send(channel, payload),
        "signal" => harmonia_signal::client::send(channel, payload),
        "mattermost" => harmonia_mattermost::client::send(channel, payload),
        "whatsapp" => harmonia_whatsapp::client::send(channel, payload),
        "nostr" => harmonia_nostr::client::send(channel, payload),
        "email" | "email-client" => harmonia_email_client::client::send(channel, payload),
        #[cfg(target_os = "macos")]
        "imessage" => harmonia_imessage::client::send(channel, payload),
        "tailscale" => harmonia_tailscale_frontend::bridge::send(channel, payload),
        _ => Err(format!("unknown frontend: {frontend}")),
    }
}

// ── Signalograd (typed API — no FFI) ─────────────────────────────────

pub(crate) fn dispatch_signalograd(
    sexp: &str,
    state: &mut harmonia_signalograd::KernelState,
) -> String {
    use harmonia_signalograd::{
        apply_feedback, parse_feedback, parse_observation, restore_state_from_path, save_state,
        simple_hash, snapshot_sexp, state_to_sexp, status_sexp, step_kernel, write_state_to_path,
        KernelState,
    };
    use std::path::PathBuf;

    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
            // Init is a no-op: the actor already owns a KernelState.
            "(:ok)".to_string()
        }
        Some("observe") => {
            let raw = extract_string(sexp, ":observation").unwrap_or_default();
            let observation = match parse_observation(&raw) {
                Ok(o) => o,
                Err(e) => return format!("(:error \"observe parse: {e}\")"),
            };
            let _projection = step_kernel(state, &observation);
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"observe save: {e}\")");
            }
            "(:ok)".to_string()
        }
        Some("status") => {
            let result = status_sexp(state);
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("snapshot") => {
            let result = snapshot_sexp(state);
            format!("(:ok :result \"{}\")", esc(&result))
        }
        Some("feedback") => {
            let raw = extract_string(sexp, ":feedback").unwrap_or_default();
            let feedback = match parse_feedback(&raw) {
                Ok(f) => f,
                Err(e) => return format!("(:error \"feedback parse: {e}\")"),
            };
            apply_feedback(state, &feedback);
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"feedback save: {e}\")");
            }
            "(:ok)".to_string()
        }
        Some("reset") => {
            *state = KernelState::new();
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"reset save: {e}\")");
            }
            "(:ok)".to_string()
        }
        Some("checkpoint") => {
            let path_str = extract_string(sexp, ":path").unwrap_or_default();
            let target = PathBuf::from(path_str.trim());
            if let Err(e) = write_state_to_path(state, &target) {
                return format!("(:error \"checkpoint failed: {e}\")");
            }
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"checkpoint save: {e}\")");
            }
            "(:ok)".to_string()
        }
        Some("restore") => {
            let path_str = extract_string(sexp, ":path").unwrap_or_default();
            let target = PathBuf::from(path_str.trim());
            match restore_state_from_path(&target) {
                Ok(restored) => {
                    *state = restored;
                    state.checkpoint_digest = simple_hash(&state_to_sexp(state));
                    if let Err(e) = save_state(state) {
                        return format!("(:error \"restore save: {e}\")");
                    }
                    "(:ok)".to_string()
                }
                Err(e) => format!("(:error \"restore failed: {e}\")"),
            }
        }
        _ => format!(
            "(:error \"unknown signalograd op: {}\")",
            op.unwrap_or_default()
        ),
    }
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
                Ok(allowed) => {
                    if harmonia_observability::harmonia_observability_is_standard() {
                        let obs_ref = harmonia_observability::get_obs_actor().cloned();
                        obs_ref.trace_event("matrix-route-decision", "chain", json!({"from": from.clone(), "to": to.clone(), "signal": signal, "noise": noise, "snr": if noise > 0.0 { signal / noise } else { signal }, "security_weight": security_weight, "dissonance": dissonance, "allowed": allowed}));
                    }
                    format!("(:ok :allowed {})", if allowed { "t" } else { "nil" })
                }
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

/// Dispatch observability commands. Called from the supervisor for init/status,
/// and as a fallback path for trace ops.
fn dispatch_observability(sexp: &str) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
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
        Some("status") => {
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
        Some(op @ "trace-start") | Some(op @ "trace-end") | Some(op @ "trace-event") => {
            dispatch_obs_trace(op, sexp);
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

/// Fast-path dispatch for observability trace ops.
/// Casts to the ObservabilityActor and returns immediately.
/// Called from ipc.rs (fire-and-forget) and dispatch_observability (fallback).
pub fn dispatch_obs_trace(op: &str, sexp: &str) {
    let obs = match harmonia_observability::get_obs_actor() {
        Some(o) => o,
        None => return,
    };
    match op {
        "trace-start" => {
            let run_id = extract_string(sexp, ":run-id").unwrap_or_default();
            let name = extract_string(sexp, ":name").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_else(|| "chain".to_string());
            let parent_run_id = extract_string(sexp, ":parent-run-id");
            let trace_id = extract_string(sexp, ":trace-id");
            let metadata_str = extract_string(sexp, ":metadata").unwrap_or_default();
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
            let run_id = extract_string(sexp, ":run-id").unwrap_or_default();
            let status = extract_string(sexp, ":status").unwrap_or_else(|| "success".to_string());
            let output_str = extract_string(sexp, ":output").unwrap_or_default();
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
            let name = extract_string(sexp, ":name").unwrap_or_default();
            let kind = extract_string(sexp, ":kind").unwrap_or_else(|| "chain".to_string());
            let metadata_str = extract_string(sexp, ":metadata").unwrap_or_default();
            let metadata_json = plist_to_json(&metadata_str);
            let metadata_val: serde_json::Value =
                serde_json::from_str(&metadata_json).unwrap_or(json!({}));
            let parent_run_id = extract_string(sexp, ":parent-run-id");
            let trace_id = extract_string(sexp, ":trace-id");
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

/// Extract the vault symbol name from a dispatch sexp (for tracing — never extracts values).
pub fn extract_vault_symbol(sexp: &str) -> String {
    extract_string(sexp, ":symbol").unwrap_or_default()
}

// ── Memory Field (typed API — no globals) ────────────────────────────

pub(crate) fn dispatch_memory_field(
    sexp: &str,
    field: &mut harmonia_memory_field::FieldState,
) -> String {
    let op = extract_keyword(sexp, ":op");
    match op.as_deref() {
        Some("init") => {
            // Init is a no-op: the actor already owns a FieldState.
            "(:ok)".to_string()
        }
        Some("load-graph") => {
            let nodes = parse_memory_field_nodes(sexp);
            let edges = parse_memory_field_edges(sexp);
            match harmonia_memory_field::load_graph(field, nodes, edges) {
                Ok(result) => result,
                Err(e) => format!("(:error \"load-graph: {e}\")"),
            }
        }
        Some("field-recall") => {
            let concepts = parse_string_list(sexp, ":query-concepts");
            let access = parse_memory_field_access_counts(sexp);
            let limit = extract_u64(sexp, ":limit") as usize;
            let limit = if limit == 0 { 10 } else { limit };
            match harmonia_memory_field::field_recall(field, concepts, access, limit) {
                Ok(result) => result,
                Err(e) => format!("(:error \"field-recall: {e}\")"),
            }
        }
        Some("step-attractors") => {
            let signal = parse_f64(sexp, ":signal");
            let noise = parse_f64(sexp, ":noise");
            match harmonia_memory_field::step_attractors(field, signal, noise) {
                Ok(result) => result,
                Err(e) => format!("(:error \"step-attractors: {e}\")"),
            }
        }
        Some("basin-status") => match harmonia_memory_field::basin_status(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"basin-status: {e}\")"),
        },
        Some("eigenmode-status") => match harmonia_memory_field::eigenmode_status(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"eigenmode-status: {e}\")"),
        },
        Some("status") => match harmonia_memory_field::status(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"status: {e}\")"),
        },
        Some("restore-basin") => {
            let basin = extract_string(sexp, ":basin").unwrap_or_else(|| "thomas-0".into());
            let energy = parse_f64(sexp, ":coercive-energy");
            let dwell = extract_u64(sexp, ":dwell-ticks");
            let threshold = parse_f64(sexp, ":threshold");
            let threshold = if threshold < 0.01 { 0.35 } else { threshold };
            match harmonia_memory_field::restore_basin(field, &basin, energy, dwell, threshold) {
                Ok(result) => result,
                Err(e) => format!("(:error \"restore-basin: {e}\")"),
            }
        }
        Some("last-field-basin") => {
            // Query Chronicle for last basin state (used by Lisp warm-start).
            match harmonia_chronicle::tables::harmonic::last_field_basin() {
                Ok((basin, energy, dwell, threshold)) => {
                    format!(
                        "(:ok :basin \"{}\" :coercive-energy {:.3} :dwell-ticks {} :threshold {:.3})",
                        basin, energy, dwell, threshold
                    )
                }
                Err(e) => format!("(:error \"last-field-basin: {e}\")"),
            }
        }
        Some("reset") => match harmonia_memory_field::reset(field) {
            Ok(result) => result,
            Err(e) => format!("(:error \"reset: {e}\")"),
        },
        _ => format!(
            "(:error \"unknown memory-field op: {}\")",
            op.unwrap_or_default()
        ),
    }
}

/// Parse node list from memory-field load-graph sexp.
/// Expected format: :nodes ((:concept "x" :domain "y" :count N :entries ("e1" "e2")) ...)
fn parse_memory_field_nodes(sexp: &str) -> Vec<(String, String, i32, Vec<String>)> {
    let mut nodes = Vec::new();
    // Find the :nodes section and extract individual node plists.
    if let Some(nodes_start) = sexp.find(":nodes") {
        let rest = &sexp[nodes_start + 6..];
        if let Some(list_start) = rest.find('(') {
            let inner = &rest[list_start..];
            // Simple parse: split by ":concept" markers.
            for chunk in inner.split(":concept").skip(1) {
                let concept = extract_first_quoted(chunk).unwrap_or_default();
                let domain = extract_after_keyword(chunk, ":domain").unwrap_or_else(|| "generic".into());
                let count = extract_after_keyword(chunk, ":count")
                    .and_then(|s| s.parse::<i32>().ok())
                    .unwrap_or(1);
                let entries = extract_string_list_inline(chunk, ":entries");
                if !concept.is_empty() {
                    nodes.push((concept, domain, count, entries));
                }
            }
        }
    }
    nodes
}

/// Parse edge list from memory-field load-graph sexp.
fn parse_memory_field_edges(sexp: &str) -> Vec<(String, String, f64, bool)> {
    let mut edges = Vec::new();
    if let Some(edges_start) = sexp.find(":edges") {
        let rest = &sexp[edges_start + 6..];
        if let Some(list_start) = rest.find('(') {
            let inner = &rest[list_start..];
            for chunk in inner.split(":a ").skip(1) {
                let a = extract_first_quoted(chunk).unwrap_or_default();
                let b = extract_after_keyword(chunk, ":b").unwrap_or_default();
                let weight = extract_after_keyword(chunk, ":weight")
                    .and_then(|s| s.parse::<f64>().ok())
                    .unwrap_or(1.0);
                let inter = extract_after_keyword(chunk, ":interdisciplinary")
                    .map(|s| s == "t")
                    .unwrap_or(false);
                if !a.is_empty() && !b.is_empty() {
                    edges.push((a, b, weight, inter));
                }
            }
        }
    }
    edges
}

/// Parse access counts from memory-field field-recall sexp.
fn parse_memory_field_access_counts(sexp: &str) -> Vec<(String, f64)> {
    let mut counts = Vec::new();
    if let Some(start) = sexp.find(":access-counts") {
        let rest = &sexp[start + 14..];
        for chunk in rest.split(":concept").skip(1) {
            let concept = extract_first_quoted(chunk).unwrap_or_default();
            let count = extract_after_keyword(chunk, ":count")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            if !concept.is_empty() {
                counts.push((concept, count));
            }
        }
    }
    counts
}

/// Parse a list of strings like ("a" "b" "c") from sexp.
fn parse_string_list(sexp: &str, key: &str) -> Vec<String> {
    extract_string_list_inline(sexp, key)
}

/// Extract the first quoted string from a text chunk.
fn extract_first_quoted(s: &str) -> Option<String> {
    let start = s.find('"')? + 1;
    let end = start + s[start..].find('"')?;
    Some(s[start..end].to_string())
}

/// Extract a value after a keyword like :domain "engineering".
fn extract_after_keyword(s: &str, key: &str) -> Option<String> {
    let pos = s.find(key)? + key.len();
    let rest = s[pos..].trim_start();
    if rest.starts_with('"') {
        extract_first_quoted(rest)
    } else {
        // Unquoted token — read until whitespace or paren.
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')' || c == '(')
            .unwrap_or(rest.len());
        let token = rest[..end].trim();
        if token.is_empty() {
            None
        } else {
            Some(token.to_string())
        }
    }
}

/// Extract a list of quoted strings after a keyword.
fn extract_string_list_inline(s: &str, key: &str) -> Vec<String> {
    let mut items = Vec::new();
    if let Some(pos) = s.find(key) {
        let rest = &s[pos + key.len()..];
        if let Some(open) = rest.find('(') {
            let inner = &rest[open + 1..];
            if let Some(close) = inner.find(')') {
                let content = &inner[..close];
                let mut in_quote = false;
                let mut current = String::new();
                for ch in content.chars() {
                    match ch {
                        '"' if !in_quote => {
                            in_quote = true;
                            current.clear();
                        }
                        '"' if in_quote => {
                            in_quote = false;
                            if !current.is_empty() {
                                items.push(current.clone());
                            }
                        }
                        _ if in_quote => current.push(ch),
                        _ => {}
                    }
                }
            }
        }
    }
    items
}
