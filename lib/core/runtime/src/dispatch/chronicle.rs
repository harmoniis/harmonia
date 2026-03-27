//! Chronicle component dispatch.

use harmonia_actor_protocol::{extract_sexp_bool, extract_sexp_f64, extract_sexp_string, sexp_escape};

fn esc(s: &str) -> String {
    sexp_escape(s)
}

pub(crate) fn dispatch(sexp: &str) -> String {
    let op = extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => match harmonia_chronicle::init() {
            Ok(_) => "(:ok)".to_string(),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "query" => {
            let sql = extract_sexp_string(sexp, ":sql").unwrap_or_default();
            match harmonia_chronicle::query_sexp(&sql) {
                Ok(result) => format!("(:ok :result \"{}\")", esc(&result)),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "harmony-summary" => match harmonia_chronicle::harmony_summary() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "dashboard" => match harmonia_chronicle::dashboard_json() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "gc" => match harmonia_chronicle::gc() {
            Ok(n) => format!("(:ok :result \"{}\")", n),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "gc-status" => match harmonia_chronicle::gc_status() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "cost-report" => match harmonia_chronicle::cost_report(0) {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "delegation-report" => match harmonia_chronicle::delegation_report() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "full-digest" => match harmonia_chronicle::full_digest() {
            Ok(s) => format!("(:ok :result \"{}\")", esc(&s)),
            Err(e) => format!("(:error \"{}\")", esc(&e)),
        },
        "record-harmonic" => {
            let snap = harmonia_chronicle::HarmonicSnapshot {
                cycle: extract_sexp_string(sexp, ":cycle")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                phase: extract_sexp_string(sexp, ":phase").unwrap_or_default(),
                strength: extract_sexp_f64(sexp, ":strength").unwrap_or(0.0),
                utility: extract_sexp_f64(sexp, ":utility").unwrap_or(0.0),
                beauty: extract_sexp_f64(sexp, ":beauty").unwrap_or(0.0),
                signal: extract_sexp_f64(sexp, ":signal").unwrap_or(0.0),
                noise: extract_sexp_f64(sexp, ":noise").unwrap_or(0.0),
                logistic_x: extract_sexp_f64(sexp, ":logistic-x").unwrap_or(0.0),
                logistic_r: extract_sexp_f64(sexp, ":logistic-r").unwrap_or(0.0),
                chaos_risk: extract_sexp_f64(sexp, ":chaos-risk").unwrap_or(0.0),
                rewrite_aggression: extract_sexp_f64(sexp, ":rewrite-aggression").unwrap_or(0.0),
                lorenz_x: extract_sexp_f64(sexp, ":lorenz-x").unwrap_or(0.0),
                lorenz_y: extract_sexp_f64(sexp, ":lorenz-y").unwrap_or(0.0),
                lorenz_z: extract_sexp_f64(sexp, ":lorenz-z").unwrap_or(0.0),
                lorenz_radius: extract_sexp_f64(sexp, ":lorenz-radius").unwrap_or(0.0),
                lorenz_bounded: extract_sexp_f64(sexp, ":lorenz-bounded").unwrap_or(0.0),
                lambdoma_global: extract_sexp_f64(sexp, ":lambdoma-global").unwrap_or(0.0),
                lambdoma_local: extract_sexp_f64(sexp, ":lambdoma-local").unwrap_or(0.0),
                lambdoma_ratio: extract_sexp_f64(sexp, ":lambdoma-ratio").unwrap_or(0.0),
                lambdoma_convergent: extract_sexp_bool(sexp, ":lambdoma-convergent").unwrap_or(false),
                rewrite_ready: extract_sexp_bool(sexp, ":rewrite-ready").unwrap_or(false),
                rewrite_count: extract_sexp_string(sexp, ":rewrite-count")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                security_posture: extract_sexp_string(sexp, ":security-posture").unwrap_or_default(),
                security_events: extract_sexp_string(sexp, ":security-events")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                field_basin: extract_sexp_string(sexp, ":field-basin").unwrap_or_else(|| "thomas-0".into()),
                field_coercive_energy: extract_sexp_f64(sexp, ":field-coercive-energy").unwrap_or(0.0),
                field_dwell_ticks: extract_sexp_string(sexp, ":field-dwell-ticks")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0),
                field_threshold: {
                    let t = extract_sexp_f64(sexp, ":field-threshold").unwrap_or(0.0);
                    if t < 0.01 { 0.35 } else { t }
                },
            };
            match harmonia_chronicle::harmonic::record(&snap) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "record-memory-event" => {
            let event_type = extract_sexp_string(sexp, ":event-type").unwrap_or_default();
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
        "persist-entry" => {
            let id = extract_sexp_string(sexp, ":id").unwrap_or_default();
            let ts: i64 = extract_sexp_string(sexp, ":ts").and_then(|s| s.parse().ok()).unwrap_or(0);
            let content = extract_sexp_string(sexp, ":content").unwrap_or_default();
            let tags = extract_sexp_string(sexp, ":tags").unwrap_or_default();
            let source_ids = extract_sexp_string(sexp, ":source-ids").unwrap_or_default();
            match harmonia_chronicle::memory::persist_entry(&id, ts, &content, &tags, &source_ids) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "load-all-entries" => {
            match harmonia_chronicle::memory::load_all_entries() {
                Ok(result) => result,
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "entry-count" => {
            match harmonia_chronicle::memory::entry_count() {
                Ok(count) => format!("(:ok :count {})", count),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "update-access" => {
            let id = extract_sexp_string(sexp, ":id").unwrap_or_default();
            match harmonia_chronicle::memory::update_access(&id) {
                Ok(_) => "(:ok)".to_string(),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "record-delegation" => {
            let task_hint = extract_sexp_string(sexp, ":task-hint");
            let model_chosen = extract_sexp_string(sexp, ":model-chosen").unwrap_or_default();
            let backend = extract_sexp_string(sexp, ":backend").unwrap_or_default();
            let reason = extract_sexp_string(sexp, ":reason");
            let escalated = extract_sexp_bool(sexp, ":escalated").unwrap_or(false);
            let escalated_from = extract_sexp_string(sexp, ":escalated-from");
            let cost_usd = extract_sexp_f64(sexp, ":cost-usd").unwrap_or(0.0);
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
        "record-graph" => {
            let source = extract_sexp_string(sexp, ":source").unwrap_or_default();
            let graph_sexp = extract_sexp_string(sexp, ":sexp").unwrap_or_default();
            match harmonia_chronicle::graph::record_snapshot(&source, &graph_sexp, &[], &[]) {
                Ok(id) => format!("(:ok :snapshot-id {})", id),
                Err(e) => format!("(:error \"{}\")", esc(&e)),
            }
        }
        "record-signalograd-event" => {
            let event_type = extract_sexp_string(sexp, ":event-type").unwrap_or_default();
            let cycle: i64 = extract_sexp_string(sexp, ":cycle")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let confidence = extract_sexp_f64(sexp, ":confidence").unwrap_or(0.0);
            let stability = extract_sexp_f64(sexp, ":stability").unwrap_or(0.0);
            let novelty = extract_sexp_f64(sexp, ":novelty").unwrap_or(0.0);
            let reward = extract_sexp_f64(sexp, ":reward").unwrap_or(0.0);
            let accepted = extract_sexp_bool(sexp, ":accepted").unwrap_or(false);
            let recall_hits: i32 = extract_sexp_string(sexp, ":recall-hits")
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let checkpoint_path = extract_sexp_string(sexp, ":checkpoint-path");
            let checkpoint_digest = extract_sexp_string(sexp, ":checkpoint-digest");
            let detail = extract_sexp_string(sexp, ":detail");
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
        "record-phoenix-event" => {
            let event_type = extract_sexp_string(sexp, ":event-type").unwrap_or_default();
            let exit_code: Option<i32> =
                extract_sexp_string(sexp, ":exit-code").and_then(|s| s.parse().ok());
            let attempt: Option<i32> =
                extract_sexp_string(sexp, ":attempt").and_then(|s| s.parse().ok());
            let max_attempts: Option<i32> =
                extract_sexp_string(sexp, ":max-attempts").and_then(|s| s.parse().ok());
            let recovery_ms: Option<i64> =
                extract_sexp_string(sexp, ":recovery-ms").and_then(|s| s.parse().ok());
            let detail = extract_sexp_string(sexp, ":detail");
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
        "record-ouroboros-event" => {
            let event_type = extract_sexp_string(sexp, ":event-type").unwrap_or_default();
            let component = extract_sexp_string(sexp, ":component");
            let detail = extract_sexp_string(sexp, ":detail");
            let patch_size: Option<i64> =
                extract_sexp_string(sexp, ":patch-size").and_then(|s| s.parse().ok());
            let success = extract_sexp_bool(sexp, ":success").unwrap_or(false);
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
        _ => format!("(:error \"unknown chronicle op: {}\")", op),
    }
}
