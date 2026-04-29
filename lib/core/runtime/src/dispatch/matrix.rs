//! Harmonic-matrix component dispatch — sync fallback + actor-based async path.

use ractor::ActorRef;
use serde_json::json;

use harmonia_actor_protocol::{extract_sexp_bool, extract_sexp_f64, extract_sexp_string};
use harmonia_observability::{ObsMsg, Traceable};

use crate::actors::MatrixMsg;

use super::esc;

/// Route matrix commands through the HarmonicMatrixActor for serialized access.
/// Observability traces matrix operations via the obs actor (no statics, no mutex).
pub async fn dispatch_matrix_via_actor(
    matrix: &ActorRef<MatrixMsg>,
    obs: &Option<ActorRef<ObsMsg>>,
    sexp: &str,
) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "register-node" => {
            // Accept both :node (Lisp convention) and :id (legacy).
            let id = extract_sexp_string(sexp, ":node")
                .or_else(|| extract_sexp_string(sexp, ":node-id"))
                .or_else(|| extract_sexp_string(sexp, ":id"))
                .unwrap_or_default();
            let kind = extract_sexp_string(sexp, ":kind").unwrap_or_default();
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
        "register-edge" => {
            let from = extract_sexp_string(sexp, ":from").unwrap_or_default();
            let to = extract_sexp_string(sexp, ":to").unwrap_or_default();
            let weight = extract_sexp_f64(sexp, ":weight").unwrap_or(0.0);
            let min_harmony = extract_sexp_f64(sexp, ":min-harmony").unwrap_or(0.0);
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
        "observe-route" => {
            let from = extract_sexp_string(sexp, ":from").unwrap_or_default();
            let to = extract_sexp_string(sexp, ":to").unwrap_or_default();
            let success = extract_sexp_bool(sexp, ":success").unwrap_or(false);
            let latency_ms = extract_sexp_string(sexp, ":latency-ms")
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            let cost_usd = extract_sexp_f64(sexp, ":cost-usd").unwrap_or(0.0);
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
        "log-event" => {
            let component = extract_sexp_string(sexp, ":component").unwrap_or_default();
            let direction = extract_sexp_string(sexp, ":direction").unwrap_or_default();
            let channel = extract_sexp_string(sexp, ":channel").unwrap_or_default();
            let payload = extract_sexp_string(sexp, ":payload").unwrap_or_default();
            let success = extract_sexp_bool(sexp, ":success").unwrap_or(false);
            let error = extract_sexp_string(sexp, ":error").unwrap_or_default();
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
        "set-tool-enabled" => {
            let node = extract_sexp_string(sexp, ":node").unwrap_or_default();
            let enabled = extract_sexp_bool(sexp, ":enabled").unwrap_or(false);
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
        "route-allowed" => {
            let from = extract_sexp_string(sexp, ":from").unwrap_or_default();
            let to = extract_sexp_string(sexp, ":to").unwrap_or_default();
            let signal = extract_sexp_f64(sexp, ":signal").unwrap_or(0.0);
            let noise = extract_sexp_f64(sexp, ":noise").unwrap_or(0.0);
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
        "report" => match ractor::call_t!(matrix, MatrixMsg::Report, 5000) {
            Ok(report) => format!("(:ok :result \"{}\")", esc(&report)),
            Err(_) => "(:error \"matrix report timeout\")".to_string(),
        },
        "store-summary" => match ractor::call_t!(matrix, MatrixMsg::StoreSummary, 5000) {
            Ok(summary) => format!("(:ok :result \"{}\")", esc(&summary)),
            Err(_) => "(:error \"matrix store-summary timeout\")".to_string(),
        },
        _ => {
            // Fall back to synchronous dispatch for ops not yet in the actor
            dispatch(sexp)
        }
    }
}

/// Synchronous matrix dispatch (fallback for ops not routed through the actor).
/// Operations that exist in the async actor path (register-node, register-edge,
/// observe-route, log-event, set-tool-enabled, route-allowed) are NOT duplicated
/// here — the supervisor always routes through dispatch_matrix_via_actor.
pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => match harmonia_harmonic_matrix::runtime::store::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "set-store" => {
            let kind = extract_sexp_string(sexp, ":kind").unwrap_or_default();
            let path = extract_sexp_string(sexp, ":path");
            match harmonia_harmonic_matrix::runtime::store::set_store(&kind, path.as_deref()) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "route-allowed-ctx" => {
            let from = extract_sexp_string(sexp, ":from").unwrap_or_default();
            let to = extract_sexp_string(sexp, ":to").unwrap_or_default();
            let signal = extract_sexp_f64(sexp, ":signal").unwrap_or(0.0);
            let noise = extract_sexp_f64(sexp, ":noise").unwrap_or(0.0);
            let security_weight = extract_sexp_f64(sexp, ":security-weight").unwrap_or(0.0);
            let dissonance = extract_sexp_f64(sexp, ":dissonance").unwrap_or(0.0);
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
                        use harmonia_observability::Traceable;
                        obs_ref.trace_event("matrix-route-decision", "chain", json!({"from": from.clone(), "to": to.clone(), "signal": signal, "noise": noise, "snr": if noise > 0.0 { signal / noise } else { signal }, "security_weight": security_weight, "dissonance": dissonance, "allowed": allowed}));
                    }
                    format!("(:ok :allowed {})", if allowed { "t" } else { "nil" })
                }
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "route-timeseries" => {
            let from = extract_sexp_string(sexp, ":from").unwrap_or_default();
            let to = extract_sexp_string(sexp, ":to").unwrap_or_default();
            let limit: i32 = extract_sexp_string(sexp, ":limit")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match harmonia_harmonic_matrix::runtime::reports::route_timeseries(&from, &to, limit) {
                Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "time-report" => {
            let since_unix: u64 = extract_sexp_string(sexp, ":since")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            match harmonia_harmonic_matrix::runtime::reports::time_report(since_unix) {
                Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "report" => match harmonia_harmonic_matrix::runtime::reports::report() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "store-summary" => match harmonia_harmonic_matrix::runtime::store::store_summary() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        _ => format!("(:error \"unknown harmonic-matrix op: {}\")", esc(&op)),
    }
}
