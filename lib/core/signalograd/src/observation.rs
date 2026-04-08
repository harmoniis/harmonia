use crate::error::clamp;
use crate::model::{Feedback, Observation, Projection, INPUT_DIM};
use crate::sexp::{
    parse_sexp, plist_bool, plist_f64, plist_i64, plist_list, plist_string, plist_view, Sexp,
};

pub fn parse_observation(raw: &str) -> Result<Observation, String> {
    if raw.trim_start().starts_with('{') {
        return serde_json::from_str(raw)
            .map_err(|e| format!("invalid signalograd observation: {e}"));
    }
    let sexp = parse_sexp(raw)?;
    parse_observation_sexp(&sexp)
}

pub fn parse_feedback(raw: &str) -> Result<Feedback, String> {
    let sexp = parse_sexp(raw)?;
    parse_feedback_sexp(&sexp)
}

pub(crate) fn parse_observation_sexp(sexp: &Sexp) -> Result<Observation, String> {
    let items = plist_view(sexp)?;
    Ok(Observation {
        cycle: plist_i64(items, "cycle").unwrap_or(0),
        global_score: plist_f64(items, "global-score").unwrap_or(0.0),
        local_score: plist_f64(items, "local-score").unwrap_or(0.0),
        signal: plist_f64(items, "signal").unwrap_or(0.0),
        noise: plist_f64(items, "noise").unwrap_or(1.0),
        chaos_risk: plist_f64(items, "chaos-risk").unwrap_or(1.0),
        rewrite_aggression: plist_f64(items, "rewrite-aggression").unwrap_or(0.0),
        lorenz_bounded: plist_f64(items, "lorenz-bounded").unwrap_or(0.0),
        lambdoma_ratio: plist_f64(items, "lambdoma-ratio").unwrap_or(0.0),
        rewrite_ready: plist_bool(items, "rewrite-ready").unwrap_or(false),
        security_posture: plist_string(items, "security-posture")
            .unwrap_or_else(|| "nominal".to_string()),
        security_events: plist_f64(items, "security-events").unwrap_or(0.0),
        route_success: plist_f64(items, "route-success").unwrap_or(0.0),
        route_latency: plist_f64(items, "route-latency").unwrap_or(0.0),
        cost_pressure: plist_f64(items, "cost-pressure").unwrap_or(0.0),
        memory_pressure: plist_f64(items, "memory-pressure").unwrap_or(0.0),
        graph_density: plist_f64(items, "graph-density").unwrap_or(0.0),
        graph_interdisciplinary: plist_f64(items, "graph-interdisciplinary").unwrap_or(0.0),
        reward: plist_f64(items, "reward").unwrap_or(0.0),
        stability: plist_f64(items, "stability").unwrap_or(0.0),
        novelty: plist_f64(items, "novelty").unwrap_or(0.0),
        actor_load: plist_f64(items, "actor-load").unwrap_or(0.0),
        actor_stalls: plist_f64(items, "actor-stalls").unwrap_or(0.0),
        queue_depth: plist_f64(items, "queue-depth").unwrap_or(0.0),
        error_pressure: plist_f64(items, "error-pressure").unwrap_or(0.0),
        supervision: plist_f64(items, "supervision").unwrap_or(0.5),
        prior_confidence: plist_f64(items, "prior-confidence").unwrap_or(0.0),
        field_recall_strength: plist_f64(items, "field-recall-strength").unwrap_or(0.0),
        field_basin_stability: plist_f64(items, "field-basin-stability").unwrap_or(0.0),
        field_eigenmode_coherence: plist_f64(items, "field-eigenmode-coherence").unwrap_or(0.0),
        presentation_cleanliness: plist_f64(items, "presentation-cleanliness").unwrap_or(1.0),
        presentation_verbosity: plist_f64(items, "presentation-verbosity").unwrap_or(0.0),
        presentation_markdown_density: plist_f64(items, "presentation-markdown-density")
            .unwrap_or(0.0),
        presentation_symbolic_density: plist_f64(items, "presentation-symbolic-density")
            .unwrap_or(0.0),
        presentation_self_reference: plist_f64(items, "presentation-self-reference").unwrap_or(0.0),
        presentation_decor_density: plist_f64(items, "presentation-decor-density").unwrap_or(0.0),
        presentation_user_affinity: plist_f64(items, "presentation-user-affinity").unwrap_or(0.5),
        route_tier: plist_string(items, "route-tier").unwrap_or_else(|| "auto".to_string()),
        datamine_success_rate: plist_f64(items, "datamine-success-rate").unwrap_or(0.0),
        datamine_avg_latency: plist_f64(items, "datamine-avg-latency").unwrap_or(0.0),
        palace_graph_density: plist_f64(items, "palace-graph-density").unwrap_or(0.0),
    })
}

pub(crate) fn parse_feedback_sexp(sexp: &Sexp) -> Result<Feedback, String> {
    let items = plist_view(sexp)?;
    Ok(Feedback {
        cycle: plist_i64(items, "cycle").unwrap_or(0),
        reward: plist_f64(items, "reward").unwrap_or(0.0),
        stability: plist_f64(items, "stability").unwrap_or(0.0),
        novelty: plist_f64(items, "novelty").unwrap_or(0.0),
        accepted: plist_bool(items, "accepted").unwrap_or(false),
        recall_hits: plist_i64(items, "recall-hits").unwrap_or(0),
        user_affinity: plist_f64(items, "user-affinity").unwrap_or(0.5),
        cleanliness: plist_f64(items, "cleanliness").unwrap_or(1.0),
        applied_confidence: plist_f64(items, "applied-confidence").unwrap_or(0.0),
    })
}

pub(crate) fn parse_projection_sexp(sexp: &Sexp) -> Result<Projection, String> {
    let items = plist_view(sexp)?;
    let harmony = plist_list(items, "harmony").unwrap_or(&[]);
    let routing = plist_list(items, "routing").unwrap_or(&[]);
    let memory = plist_list(items, "memory").unwrap_or(&[]);
    let security = plist_list(items, "security-shell").unwrap_or(&[]);
    let presentation = plist_list(items, "presentation").unwrap_or(&[]);

    Ok(Projection {
        cycle: plist_i64(items, "cycle").unwrap_or(0),
        confidence: plist_f64(items, "confidence").unwrap_or(0.0),
        stability: plist_f64(items, "stability").unwrap_or(0.0),
        novelty: plist_f64(items, "novelty").unwrap_or(0.0),
        latent_energy: plist_f64(items, "latent-energy").unwrap_or(0.0),
        recall_strength: plist_f64(items, "recall-strength").unwrap_or(0.0),
        harmony_signal_bias: plist_f64(harmony, "signal-bias").unwrap_or(0.0),
        harmony_noise_bias: plist_f64(harmony, "noise-bias").unwrap_or(0.0),
        rewrite_signal_delta: plist_f64(harmony, "rewrite-signal-delta").unwrap_or(0.0),
        rewrite_chaos_delta: plist_f64(harmony, "rewrite-chaos-delta").unwrap_or(0.0),
        evolution_aggression_bias: plist_f64(harmony, "aggression-bias").unwrap_or(0.0),
        routing_price_delta: plist_f64(routing, "price-weight-delta").unwrap_or(0.0),
        routing_speed_delta: plist_f64(routing, "speed-weight-delta").unwrap_or(0.0),
        routing_success_delta: plist_f64(routing, "success-weight-delta").unwrap_or(0.0),
        routing_reasoning_delta: plist_f64(routing, "reasoning-weight-delta").unwrap_or(0.0),
        routing_vitruvian_min_delta: plist_f64(routing, "vitruvian-min-delta").unwrap_or(0.0),
        memory_recall_limit_delta: plist_i64(memory, "recall-limit-delta").unwrap_or(0) as i32,
        memory_crystal_threshold_delta: plist_f64(memory, "crystal-threshold-delta").unwrap_or(0.0),
        security_dissonance_delta: plist_f64(security, "dissonance-weight-delta").unwrap_or(0.0),
        security_anomaly_delta: plist_f64(security, "anomaly-threshold-delta").unwrap_or(0.0),
        presentation_verbosity_delta: plist_f64(presentation, "verbosity-delta").unwrap_or(0.0),
        presentation_markdown_density_delta: plist_f64(presentation, "markdown-density-delta")
            .unwrap_or(0.0),
        presentation_symbolic_density_delta: plist_f64(presentation, "symbolic-density-delta")
            .unwrap_or(0.0),
        presentation_self_reference_delta: plist_f64(presentation, "self-reference-delta")
            .unwrap_or(0.0),
        presentation_decor_density_delta: plist_f64(presentation, "decor-density-delta")
            .unwrap_or(0.0),
    })
}

pub fn observation_vector(obs: &Observation) -> [f64; INPUT_DIM] {
    [
        obs.global_score,
        obs.local_score,
        obs.signal,
        obs.noise,
        obs.chaos_risk,
        obs.rewrite_aggression,
        obs.lorenz_bounded,
        obs.lambdoma_ratio,
        if obs.rewrite_ready { 1.0 } else { 0.0 },
        posture_scalar(obs.security_posture.trim()),
        clamp(obs.security_events / 12.0, 0.0, 1.0),
        obs.route_success,
        obs.route_latency,
        obs.cost_pressure,
        obs.memory_pressure,
        obs.graph_density,
        obs.graph_interdisciplinary,
        obs.reward,
        0.5 * obs.stability + 0.5 * obs.novelty,
        clamp(obs.actor_load / 8.0, 0.0, 1.0),
        clamp(obs.actor_stalls / 8.0, 0.0, 1.0),
        clamp(obs.queue_depth / 12.0, 0.0, 1.0),
        clamp(obs.error_pressure, 0.0, 1.0),
        clamp(obs.supervision, 0.0, 1.0),
        clamp(obs.prior_confidence, 0.0, 1.0),
        // Memory-field feedback (3 dimensions).
        clamp(obs.field_recall_strength, 0.0, 1.0),
        clamp(obs.field_basin_stability, 0.0, 1.0),
        clamp(obs.field_eigenmode_coherence, 0.0, 1.0),
        // Datamining feedback (3 dimensions: terraphon + mempalace).
        clamp(obs.datamine_success_rate, 0.0, 1.0),
        clamp(obs.datamine_avg_latency / 5000.0, 0.0, 1.0), // normalize to DNA max-latency
        clamp(obs.palace_graph_density, 0.0, 1.0),
    ]
}

pub fn posture_scalar(posture: &str) -> f64 {
    match posture {
        "alert" => 1.0,
        "elevated" => 0.6,
        "nominal" => 0.1,
        _ => 0.25,
    }
}
