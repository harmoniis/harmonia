use std::collections::HashMap;

use crate::model::MatrixEvent;

use super::shared::{state, store_config};


pub fn route_timeseries(from: &str, to: &str, limit: i32) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let samples = st
        .route_history
        .get(&(from.to_string(), to.to_string()))
        .cloned()
        .unwrap_or_default();

    let n = if limit <= 0 {
        samples.len()
    } else {
        limit as usize
    };
    let start = samples.len().saturating_sub(n);
    let out: Vec<String> = samples[start..]
        .iter()
        .map(|s| {
            format!(
                "(:ts {} :success {} :latency-ms {} :cost-usd {:.8})",
                s.ts,
                if s.success { "t" } else { "nil" },
                s.latency_ms,
                s.cost_usd
            )
        })
        .collect();

    Ok(format!(
        "(:from \"{}\" :to \"{}\" :samples ({}))",
        from,
        to,
        out.join(" ")
    ))
}


pub fn time_report(since_unix: u64) -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();

    let events: Vec<&MatrixEvent> = st.events.iter().filter(|e| e.ts >= since_unix).collect();
    let mut by_component: HashMap<String, (u64, u64)> = HashMap::new();
    for e in &events {
        let entry = by_component.entry(e.component.clone()).or_insert((0, 0));
        entry.0 += 1;
        if e.success {
            entry.1 += 1;
        }
    }

    let mut components = Vec::new();
    for (c, (count, ok)) in by_component {
        let sr = if count == 0 {
            0.0
        } else {
            ok as f64 / count as f64
        };
        components.push(format!(
            "(:component \"{}\" :count {} :success-rate {:.4})",
            c, count, sr
        ));
    }
    components.sort();

    let mut recent_events = Vec::new();
    for e in events.iter().rev().take(20).rev() {
        recent_events.push(format!(
            "(:ts {} :component \"{}\" :direction \"{}\" :channel \"{}\" :success {} :payload \"{}\" :error \"{}\")",
            e.ts,
            e.component,
            e.direction,
            e.channel,
            if e.success { "t" } else { "nil" },
            e.payload.replace('"', "\\\""),
            e.error.replace('"', "\\\"")
        ));
    }

    Ok(format!(
        "(:store-kind \"{}\" :store-path \"{}\" :epoch {} :revision {} :since {} :event-count {} :components ({}) :recent-events ({}))",
        cfg.kind_name(),
        cfg.path.replace('"', "\\\""),
        st.epoch,
        st.revision,
        since_unix,
        events.len(),
        components.join(" "),
        recent_events.join(" ")
    ))
}


pub fn report() -> Result<String, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();

    let mut node_entries: Vec<String> = st
        .nodes
        .iter()
        .map(|(id, kind)| {
            let plugged = if kind == "tool" {
                st.plugged.get(id).copied().unwrap_or(true)
            } else {
                true
            };
            format!(
                "(:id \"{}\" :kind :{} :plugged {})",
                id,
                kind,
                if plugged { "t" } else { "nil" }
            )
        })
        .collect();
    node_entries.sort();

    let mut edge_entries: Vec<String> = st
        .edges
        .iter()
        .map(|((from, to), e)| {
            let sr = if e.uses == 0 {
                0.0
            } else {
                e.successes as f64 / e.uses as f64
            };
            let avg_latency = if e.uses == 0 {
                0.0
            } else {
                e.total_latency_ms as f64 / e.uses as f64
            };
            let hist = st
                .route_history
                .get(&(from.clone(), to.clone()))
                .map(|h| h.len())
                .unwrap_or(0);
            format!(
                "(:from \"{}\" :to \"{}\" :weight {:.4} :min-harmony {:.4} :uses {} :success-rate {:.4} :avg-latency-ms {:.2} :total-cost-usd {:.8} :history {})",
                from, to, e.weight, e.min_harmony, e.uses, sr, avg_latency, e.total_cost_usd, hist
            )
        })
        .collect();
    edge_entries.sort();

    Ok(format!(
        "(:store-kind \"{}\" :store-path \"{}\" :epoch {} :revision {} :event-count {} :nodes ({}) :edges ({}))",
        cfg.kind_name(),
        cfg.path.replace('"', "\\\""),
        st.epoch,
        st.revision,
        st.events.len(),
        node_entries.join(" "),
        edge_entries.join(" ")
    ))
}
