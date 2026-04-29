use crate::model::{Edge, MatrixEvent, RouteSample, State};

use super::shared::{
    bump_revision, history_limit, now_unix, push_limited, state, truncate_payload,
};
use super::store::persist_if_needed;

/// Summary returned by epoch advancement.
pub struct EpochSummary {
    pub epoch: u64,
    pub samples_retained: usize,
    pub edges_with_history: usize,
}

impl EpochSummary {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:ok :epoch {} :samples-retained {} :edges-with-history {})",
            self.epoch, self.samples_retained, self.edges_with_history,
        )
    }
}

/// Advance the matrix epoch: bump the counter, age the rolling route-sample
/// histories down to a tight window so long-running processes don't grow
/// unbounded, persist if needed.
///
/// Called from `HarmonicMatrixActor::Tick`. Cheap when `route_history` is small;
/// only does real work for edges whose sample buffer has crossed the threshold.
pub fn advance_epoch() -> Result<EpochSummary, String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
    st.epoch = st.epoch.saturating_add(1);
    // Tighter retention than the per-insert limit: each tick prunes back to
    // 1/4 of the configured window so old behaviour decays out under sustained
    // traffic. The per-insert push_limited still bounds spikes.
    let retain_per_edge = history_limit() / 4;
    let mut samples_retained: usize = 0;
    let mut edges_with_history: usize = 0;
    for samples in st.route_history.values_mut() {
        if samples.len() > retain_per_edge {
            let drop = samples.len() - retain_per_edge;
            samples.drain(0..drop);
        }
        if !samples.is_empty() {
            edges_with_history += 1;
            samples_retained += samples.len();
        }
    }
    bump_revision(&mut st);
    persist_if_needed(&st)?;
    Ok(EpochSummary {
        epoch: st.epoch,
        samples_retained,
        edges_with_history,
    })
}


fn tool_allowed(st: &State, node_id: &str) -> bool {
    if st.nodes.get(node_id).map(|k| k.as_str()) != Some("tool") {
        return true;
    }
    st.plugged.get(node_id).copied().unwrap_or(true)
}


fn validate_node_kind(kind: &str) -> Result<(), String> {
    match kind {
        "core" | "backend" | "tool" => Ok(()),
        _ => Err(format!(
            "invalid node kind: {} (must be core, backend, or tool)",
            kind
        )),
    }
}


pub fn register_node(node_id: &str, kind: &str) -> Result<(), String> {
    validate_node_kind(kind)?;

    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    st.nodes.insert(node_id.to_string(), kind.to_string());
    if kind == "tool" {
        st.plugged.entry(node_id.to_string()).or_insert(true);
    }
    bump_revision(&mut st);
    persist_if_needed(&st)
}


pub fn set_tool_enabled(tool_id: &str, enabled: bool) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    match st.nodes.get(tool_id) {
        Some(kind) if kind == "tool" => {
            st.plugged.insert(tool_id.to_string(), enabled);
            bump_revision(&mut st);
            persist_if_needed(&st)
        }
        _ => Err("tool not registered or not kind=tool".to_string()),
    }
}


pub fn register_edge(from: &str, to: &str, weight: f64, min_harmony: f64) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    if !st.nodes.contains_key(from) || !st.nodes.contains_key(to) {
        return Err("both nodes must be registered before edge registration".to_string());
    }

    st.edges.insert(
        (from.to_string(), to.to_string()),
        Edge {
            weight,
            min_harmony,
            uses: 0,
            successes: 0,
            total_latency_ms: 0,
            total_cost_usd: 0.0,
        },
    );

    bump_revision(&mut st);
    persist_if_needed(&st)
}


pub fn route_allowed(from: &str, to: &str, signal: f64, noise: f64) -> Result<bool, String> {
    route_allowed_with_context(from, to, signal, noise, 1.0, 0.0)
}

/// Wave 3.2: Security-aware routing with dissonance and security weight.
/// This is the adaptive shell's routing layer — defense-in-depth alongside the kernel's policy gate.

pub fn route_allowed_with_context(
    from: &str,
    to: &str,
    signal: f64,
    noise: f64,
    security_weight: f64,
    dissonance: f64,
) -> Result<bool, String> {
    let st = state()
        .read()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    // Open policy: if no edge is registered, allow by default.
    // The matrix enforces constraints only when topology is explicitly configured.
    let edge = match st.edges.get(&(from.to_string(), to.to_string())) {
        Some(e) => e,
        None => return Ok(true),
    };

    if !tool_allowed(&st, from) || !tool_allowed(&st, to) {
        return Err(format!(
            "route denied: unplugged tool on {} -> {}",
            from, to
        ));
    }

    let effective_signal = signal * security_weight;
    let effective_noise = noise + dissonance;
    let harmonic_signal = effective_signal - effective_noise + edge.weight;
    let allowed = effective_signal >= effective_noise && harmonic_signal >= edge.min_harmony;
    if !allowed {
        return Err(format!(
            "route denied by harmonic threshold {} -> {} (signal={:.4} noise={:.4} sec_weight={:.4} dissonance={:.4} weight={:.4} min={:.4})",
            from, to, signal, noise, security_weight, dissonance, edge.weight, edge.min_harmony
        ));
    }

    Ok(true)
}


pub fn observe_route(
    from: &str,
    to: &str,
    success: bool,
    latency_ms: u64,
    cost_usd: f64,
) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let edge = st
        .edges
        .get_mut(&(from.to_string(), to.to_string()))
        .ok_or_else(|| format!("route observe failed: edge missing {} -> {}", from, to))?;

    edge.uses += 1;
    if success {
        edge.successes += 1;
    }
    edge.total_latency_ms += latency_ms;
    edge.total_cost_usd += cost_usd.max(0.0);

    let key = (from.to_string(), to.to_string());
    let sample = RouteSample {
        ts: now_unix(),
        success,
        latency_ms,
        cost_usd: cost_usd.max(0.0),
    };
    let limit = history_limit();
    let samples = st.route_history.entry(key).or_default();
    push_limited(samples, sample, limit);
    bump_revision(&mut st);

    persist_if_needed(&st)
}


pub fn log_event(
    component: &str,
    direction: &str,
    channel: &str,
    payload: &str,
    success: bool,
    error: &str,
) -> Result<(), String> {
    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    let event = MatrixEvent {
        ts: now_unix(),
        component: component.to_string(),
        direction: direction.to_string(),
        channel: channel.to_string(),
        payload: truncate_payload(payload, 512),
        success,
        error: truncate_payload(error, 512),
    };
    let limit = history_limit();
    push_limited(&mut st.events, event, limit);
    bump_revision(&mut st);

    persist_if_needed(&st)
}
