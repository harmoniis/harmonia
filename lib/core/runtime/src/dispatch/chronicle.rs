//! Chronicle component dispatch — pure functional, declarative.

use harmonia_actor_protocol::{extract_sexp_bool, extract_sexp_string, sexp_escape};

use super::{dispatch_op, param, param_f64, param_u64};

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => dispatch_op!("init",
            harmonia_chronicle::init().map(|_| "(:ok)".to_string())),
        "query" => dispatch_op!("query", {
            let sql = param!(sexp, ":sql");
            harmonia_chronicle::query_sexp(&sql)
                .map(|result| format!("(:ok :result \"{}\")", sexp_escape(&result)))
        }),
        "harmony-summary" => dispatch_op!("harmony-summary",
            harmonia_chronicle::harmony_summary()
                .map(|s| format!("(:ok :result \"{}\")", sexp_escape(&s)))),
        "dashboard" => dispatch_op!("dashboard",
            harmonia_chronicle::dashboard_json()
                .map(|s| format!("(:ok :result \"{}\")", sexp_escape(&s)))),
        "gc" => dispatch_op!("gc",
            harmonia_chronicle::gc()
                .map(|n| format!("(:ok :result \"{}\")", n))),
        "gc-status" => dispatch_op!("gc-status",
            harmonia_chronicle::gc_status()
                .map(|s| format!("(:ok :result \"{}\")", sexp_escape(&s)))),
        "cost-report" => dispatch_op!("cost-report",
            harmonia_chronicle::cost_report(0)
                .map(|s| format!("(:ok :result \"{}\")", sexp_escape(&s)))),
        "delegation-report" => dispatch_op!("delegation-report",
            harmonia_chronicle::delegation_report()
                .map(|s| format!("(:ok :result \"{}\")", sexp_escape(&s)))),
        "full-digest" => dispatch_op!("full-digest",
            harmonia_chronicle::full_digest()
                .map(|s| format!("(:ok :result \"{}\")", sexp_escape(&s)))),
        "record-harmonic" => dispatch_op!("record-harmonic", {
            let snap = harmonia_chronicle::HarmonicSnapshot {
                cycle: extract_sexp_string(sexp, ":cycle")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                phase: param!(sexp, ":phase"),
                strength: param_f64!(sexp, ":strength", 0.0),
                utility: param_f64!(sexp, ":utility", 0.0),
                beauty: param_f64!(sexp, ":beauty", 0.0),
                signal: param_f64!(sexp, ":signal", 0.0),
                noise: param_f64!(sexp, ":noise", 0.0),
                logistic_x: param_f64!(sexp, ":logistic-x", 0.0),
                logistic_r: param_f64!(sexp, ":logistic-r", 0.0),
                chaos_risk: param_f64!(sexp, ":chaos-risk", 0.0),
                rewrite_aggression: param_f64!(sexp, ":rewrite-aggression", 0.0),
                lorenz_x: param_f64!(sexp, ":lorenz-x", 0.0),
                lorenz_y: param_f64!(sexp, ":lorenz-y", 0.0),
                lorenz_z: param_f64!(sexp, ":lorenz-z", 0.0),
                lorenz_radius: param_f64!(sexp, ":lorenz-radius", 0.0),
                lorenz_bounded: param_f64!(sexp, ":lorenz-bounded", 0.0),
                lambdoma_global: param_f64!(sexp, ":lambdoma-global", 0.0),
                lambdoma_local: param_f64!(sexp, ":lambdoma-local", 0.0),
                lambdoma_ratio: param_f64!(sexp, ":lambdoma-ratio", 0.0),
                lambdoma_convergent: extract_sexp_bool(sexp, ":lambdoma-convergent").unwrap_or(false),
                rewrite_ready: extract_sexp_bool(sexp, ":rewrite-ready").unwrap_or(false),
                rewrite_count: extract_sexp_string(sexp, ":rewrite-count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                security_posture: param!(sexp, ":security-posture"),
                security_events: extract_sexp_string(sexp, ":security-events")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                field_basin: param!(sexp, ":field-basin", "thomas-0"),
                field_coercive_energy: param_f64!(sexp, ":field-coercive-energy", 0.0),
                field_dwell_ticks: extract_sexp_string(sexp, ":field-dwell-ticks")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                field_threshold: {
                    let t = param_f64!(sexp, ":field-threshold", 0.0);
                    if t < 0.01 { 0.35 } else { t }
                },
            };
            harmonia_chronicle::harmonic::record(&snap).map(|_| "(:ok)".to_string())
        }),
        "update-field-checkpoint" => dispatch_op!("update-field-checkpoint", {
            let checkpoint = param!(sexp, ":checkpoint");
            harmonia_chronicle::harmonic::update_field_checkpoint(&checkpoint)
                .map(|_| "(:ok)".to_string())
        }),
        "record-memory-event" => dispatch_op!("record-memory-event", {
            let event_type = param!(sexp, ":event-type");
            let entries_created: i32 = extract_sexp_string(sexp, ":entries-created")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let entries_source: i32 = extract_sexp_string(sexp, ":entries-source")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let old_size: i64 = extract_sexp_string(sexp, ":old-size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let new_size: i64 = extract_sexp_string(sexp, ":new-size")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let node_count: i32 = extract_sexp_string(sexp, ":node-count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let edge_count: i32 = extract_sexp_string(sexp, ":edge-count")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let interdisciplinary_edges: i32 = extract_sexp_string(sexp, ":interdisciplinary-edges")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let max_depth: i32 = extract_sexp_string(sexp, ":max-depth")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let detail = extract_sexp_string(sexp, ":detail");
            harmonia_chronicle::memory::record(
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
            ).map(|_| "(:ok)".to_string())
        }),
        "persist-entry" => dispatch_op!("persist-entry", {
            let id = param!(sexp, ":id");
            let ts: i64 = extract_sexp_string(sexp, ":ts").and_then(|s| s.parse().ok()).unwrap_or(0);
            let content = param!(sexp, ":content");
            let tags = param!(sexp, ":tags");
            let source_ids = param!(sexp, ":source-ids");
            harmonia_chronicle::memory::persist_entry(&id, ts, &content, &tags, &source_ids)
                .map(|_| "(:ok)".to_string())
        }),
        "load-all-entries" => dispatch_op!("load-all-entries", harmonia_chronicle::memory::load_all_entries()),
        "entry-count" => dispatch_op!("entry-count",
            harmonia_chronicle::memory::entry_count()
                .map(|count| format!("(:ok :count {})", count))),
        "update-access" => dispatch_op!("update-access", {
            let id = param!(sexp, ":id");
            harmonia_chronicle::memory::update_access(&id)
                .map(|_| "(:ok)".to_string())
        }),
        "record-delegation" => dispatch_op!("record-delegation", {
            let task_hint = extract_sexp_string(sexp, ":task-hint");
            let model_chosen = param!(sexp, ":model-chosen");
            let backend = param!(sexp, ":backend");
            let reason = extract_sexp_string(sexp, ":reason");
            let escalated = extract_sexp_bool(sexp, ":escalated").unwrap_or(false);
            let escalated_from = extract_sexp_string(sexp, ":escalated-from");
            let cost_usd = param_f64!(sexp, ":cost-usd", 0.0);
            let latency_ms: i64 = extract_sexp_string(sexp, ":latency-ms")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let success = extract_sexp_bool(sexp, ":success").unwrap_or(false);
            let tokens_in: i64 = extract_sexp_string(sexp, ":tokens-in")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let tokens_out: i64 = extract_sexp_string(sexp, ":tokens-out")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            harmonia_chronicle::delegation::record(
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
            ).map(|_| "(:ok)".to_string())
        }),
        "record-graph" => dispatch_op!("record-graph", {
            let source = param!(sexp, ":source");
            let graph_sexp = param!(sexp, ":sexp");
            harmonia_chronicle::graph::record_snapshot(&source, &graph_sexp, &[], &[])
                .map(|id| format!("(:ok :snapshot-id {})", id))
        }),
        "record-signalograd-event" => dispatch_op!("record-signalograd-event", {
            let event_type = param!(sexp, ":event-type");
            let cycle: i64 = extract_sexp_string(sexp, ":cycle")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let confidence = param_f64!(sexp, ":confidence", 0.0);
            let stability = param_f64!(sexp, ":stability", 0.0);
            let novelty = param_f64!(sexp, ":novelty", 0.0);
            let reward = param_f64!(sexp, ":reward", 0.0);
            let accepted = extract_sexp_bool(sexp, ":accepted").unwrap_or(false);
            let recall_hits: i32 = extract_sexp_string(sexp, ":recall-hits")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let checkpoint_path = extract_sexp_string(sexp, ":checkpoint-path");
            let checkpoint_digest = extract_sexp_string(sexp, ":checkpoint-digest");
            let detail = extract_sexp_string(sexp, ":detail");
            harmonia_chronicle::signalograd::record(
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
            ).map(|_| "(:ok)".to_string())
        }),
        "record-phoenix-event" => dispatch_op!("record-phoenix-event", {
            let event_type = param!(sexp, ":event-type");
            let exit_code: Option<i32> =
                extract_sexp_string(sexp, ":exit-code").and_then(|s| s.parse().ok());
            let attempt: Option<i32> =
                extract_sexp_string(sexp, ":attempt").and_then(|s| s.parse().ok());
            let max_attempts: Option<i32> =
                extract_sexp_string(sexp, ":max-attempts").and_then(|s| s.parse().ok());
            let recovery_ms: Option<i64> =
                extract_sexp_string(sexp, ":recovery-ms").and_then(|s| s.parse().ok());
            let detail = extract_sexp_string(sexp, ":detail");
            harmonia_chronicle::phoenix::record(
                &event_type,
                exit_code,
                attempt,
                max_attempts,
                recovery_ms,
                detail.as_deref(),
            ).map(|_| "(:ok)".to_string())
        }),
        "record-ouroboros-event" => dispatch_op!("record-ouroboros-event", {
            let event_type = param!(sexp, ":event-type");
            let component = extract_sexp_string(sexp, ":component");
            let detail = extract_sexp_string(sexp, ":detail");
            let patch_size: Option<i64> =
                extract_sexp_string(sexp, ":patch-size").and_then(|s| s.parse().ok());
            let success = extract_sexp_bool(sexp, ":success").unwrap_or(false);
            harmonia_chronicle::ouroboros::record(
                &event_type,
                component.as_deref(),
                detail.as_deref(),
                patch_size,
                success,
            ).map(|_| "(:ok)".to_string())
        }),
        "record-error" => dispatch_op!("record-error", {
            let source = param!(sexp, ":source");
            let kind = param!(sexp, ":kind");
            let model = extract_sexp_string(sexp, ":model");
            let detail = extract_sexp_string(sexp, ":detail");
            let latency_ms = param_u64!(sexp, ":latency-ms", 0) as i64;
            let cascaded_to = extract_sexp_string(sexp, ":cascaded-to");
            harmonia_chronicle::error::record(
                &source, &kind, model.as_deref(), detail.as_deref(),
                latency_ms, cascaded_to.as_deref(),
            ).map(|_| "(:ok)".to_string())
        }),
        _ => format!("(:error \"unknown chronicle op: {}\")", op),
    }
}
