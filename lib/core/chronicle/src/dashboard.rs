use serde_json::{json, Value};

use crate::db;

/// Generate an A2UI Composite dashboard as JSON.
pub fn dashboard_json() -> Result<String, String> {
    let db_handle = db::conn()?;
    let lock = db_handle.lock().map_err(|e| e.to_string())?;

    // 1. Harmony Overview
    let harmony_overview = {
        let mut stmt = lock
            .prepare(
                "SELECT phase, strength, utility, beauty, signal, noise,
                        chaos_risk, lorenz_bounded, lambdoma_ratio,
                        security_posture, security_events, rewrite_count, cycle
                 FROM harmonic_snapshots
                 ORDER BY ts DESC LIMIT 1",
            )
            .map_err(|e| e.to_string())?;
        let text = stmt
            .query_row([], |row| {
                let phase: String = row.get(0)?;
                let strength: f64 = row.get(1)?;
                let utility: f64 = row.get(2)?;
                let beauty: f64 = row.get(3)?;
                let signal: f64 = row.get(4)?;
                let chaos: f64 = row.get(6)?;
                let posture: String = row.get(9)?;
                let rewrites: i32 = row.get(11)?;
                let cycle: i64 = row.get(12)?;
                Ok(format!(
                    "Signal: {:.3}  |  Strength: {:.3}  Utility: {:.3}  Beauty: {:.3}\n\
                     Phase: {}  |  Chaos: {:.3}  |  Security: {}  |  Cycle: {}  |  Rewrites: {}",
                    signal, strength, utility, beauty, phase, chaos, posture, cycle, rewrites
                ))
            })
            .unwrap_or_else(|_| "No harmonic data recorded yet.".to_string());
        json!({
            "type": "TextBubble",
            "props": { "text": text, "variant": "info" }
        })
    };

    // 2. Harmonic Phase Progress
    let phase_progress = {
        let phases = [
            "observe", "evaluate-global", "evaluate-local", "logistic-balance",
            "lambdoma-project", "attractor-sync", "rewrite-plan", "security-audit", "stabilize",
        ];
        let current: String = lock
            .query_row(
                "SELECT phase FROM harmonic_snapshots ORDER BY ts DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap_or_else(|_| "observe".to_string());
        let current_idx = phases.iter().position(|p| *p == current).unwrap_or(0);
        let steps: Vec<Value> = phases
            .iter()
            .enumerate()
            .map(|(i, p)| {
                json!({
                    "label": p,
                    "status": if i < current_idx { "complete" }
                              else if i == current_idx { "current" }
                              else { "pending" }
                })
            })
            .collect();
        json!({
            "type": "ProgressTracker",
            "props": { "steps": steps }
        })
    };

    // 3. Harmony Trajectory Table
    let trajectory_table = {
        let mut stmt = lock
            .prepare(
                "SELECT bucket_ts, avg_signal, avg_chaos_risk,
                        avg_strength, avg_utility, avg_beauty, sample_count
                 FROM harmony_trajectory
                 ORDER BY bucket_ts DESC LIMIT 20",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Value> = stmt
            .query_map([], |row| {
                let ts: i64 = row.get(0)?;
                Ok(json!([
                    ts / 1000,
                    format!("{:.3}", row.get::<_, f64>(1)?),
                    format!("{:.3}", row.get::<_, f64>(2)?),
                    format!("{:.3}", row.get::<_, f64>(3)?),
                    format!("{:.3}", row.get::<_, f64>(4)?),
                    format!("{:.3}", row.get::<_, f64>(5)?),
                    row.get::<_, i32>(6)?
                ]))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        json!({
            "type": "ListTable",
            "props": {
                "headers": ["Time", "Signal", "Chaos", "Strength", "Utility", "Beauty", "Samples"],
                "rows": rows,
                "sortable": true
            }
        })
    };

    // 4. Model Delegation Table
    let delegation_table = {
        let mut stmt = lock
            .prepare(
                "SELECT model_chosen,
                        COUNT(*) AS uses,
                        ROUND(SUM(cost_usd), 4) AS cost,
                        ROUND(AVG(latency_ms)) AS avg_lat,
                        ROUND(100.0 * SUM(success) / COUNT(*), 1) AS success_pct,
                        SUM(escalated) AS esc
                 FROM delegation_log
                 GROUP BY model_chosen
                 ORDER BY uses DESC
                 LIMIT 15",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Value> = stmt
            .query_map([], |row| {
                Ok(json!([
                    row.get::<_, String>(0)?,
                    row.get::<_, i32>(1)?,
                    format!("${:.4}", row.get::<_, f64>(2)?),
                    format!("{}ms", row.get::<_, i64>(3)?),
                    format!("{}%", row.get::<_, f64>(4)?),
                    row.get::<_, i32>(5)?
                ]))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        json!({
            "type": "ListTable",
            "props": {
                "headers": ["Model", "Uses", "Cost", "Avg Latency", "Success%", "Escalations"],
                "rows": rows,
                "sortable": true
            }
        })
    };

    // 5. Memory Evolution Table
    let memory_table = {
        let mut stmt = lock
            .prepare(
                "SELECT ts, event_type, entries_created, node_count,
                        edge_count, interdisciplinary_edges
                 FROM memory_events
                 ORDER BY ts DESC LIMIT 15",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Value> = stmt
            .query_map([], |row| {
                Ok(json!([
                    row.get::<_, i64>(0)? / 1000,
                    row.get::<_, String>(1)?,
                    row.get::<_, i32>(2)?,
                    row.get::<_, i32>(3)?,
                    row.get::<_, i32>(4)?,
                    row.get::<_, i32>(5)?
                ]))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        json!({
            "type": "ListTable",
            "props": {
                "headers": ["Time", "Event", "Entries", "Nodes", "Edges", "Interdisciplinary"],
                "rows": rows,
                "sortable": true
            }
        })
    };

    // 6. Phoenix/Ouroboros Activity
    let lifecycle_table = {
        let mut stmt = lock
            .prepare(
                "SELECT ts, 'phoenix' AS source, event_type, detail FROM phoenix_events
                 UNION ALL
                 SELECT ts, 'ouroboros' AS source, event_type, detail FROM ouroboros_events
                 ORDER BY ts DESC LIMIT 20",
            )
            .map_err(|e| e.to_string())?;
        let rows: Vec<Value> = stmt
            .query_map([], |row| {
                Ok(json!([
                    row.get::<_, i64>(0)? / 1000,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?.unwrap_or_default()
                ]))
            })
            .map_err(|e| e.to_string())?
            .filter_map(|r| r.ok())
            .collect();
        json!({
            "type": "ListTable",
            "props": {
                "headers": ["Time", "Source", "Event", "Detail"],
                "rows": rows,
                "sortable": true
            }
        })
    };

    // 7. Cost Summary
    let cost_summary = {
        let now_ms: i64 = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as i64;
        let day = 86_400_000_i64;

        let cost_24h: f64 = lock
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= ?1",
                rusqlite::params![now_ms - day],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        let cost_7d: f64 = lock
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= ?1",
                rusqlite::params![now_ms - 7 * day],
                |row| row.get(0),
            )
            .unwrap_or(0.0);
        let cost_30d: f64 = lock
            .query_row(
                "SELECT COALESCE(SUM(cost_usd), 0.0) FROM delegation_log WHERE ts >= ?1",
                rusqlite::params![now_ms - 30 * day],
                |row| row.get(0),
            )
            .unwrap_or(0.0);

        json!({
            "type": "TextBubble",
            "props": {
                "text": format!(
                    "Cost  24h: ${:.4}  |  7d: ${:.4}  |  30d: ${:.4}",
                    cost_24h, cost_7d, cost_30d
                ),
                "variant": "info"
            }
        })
    };

    // 8. Graph Summary
    let graph_summary = {
        let graph_text: String = lock
            .query_row(
                "SELECT node_count, edge_count, interdisciplinary_edges
                 FROM graph_snapshots ORDER BY ts DESC LIMIT 1",
                [],
                |row| {
                    let nodes: i32 = row.get(0)?;
                    let edges: i32 = row.get(1)?;
                    let inter: i32 = row.get(2)?;
                    Ok(format!(
                        "Knowledge Graph: {} nodes, {} edges ({} interdisciplinary)",
                        nodes, edges, inter
                    ))
                },
            )
            .unwrap_or_else(|_| "No graph snapshots recorded yet.".to_string());
        json!({
            "type": "TextBubble",
            "props": { "text": graph_text, "variant": "info" }
        })
    };

    // Compose
    let composite = json!({
        "type": "Composite",
        "props": {
            "layout": "vertical",
            "children": [
                harmony_overview,
                phase_progress,
                graph_summary,
                trajectory_table,
                delegation_table,
                memory_table,
                lifecycle_table,
                cost_summary,
            ]
        }
    });

    serde_json::to_string(&composite).map_err(|e| e.to_string())
}
