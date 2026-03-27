use crate::error::digest_hex;
use crate::model::{Feedback, KernelState, Observation, Projection};

pub fn escape_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

pub fn format_f64(value: f64) -> String {
    format!("{value:.9}")
}

pub fn vector_to_sexp(values: &[f64]) -> String {
    let body = values
        .iter()
        .map(|value| format_f64(*value))
        .collect::<Vec<_>>()
        .join(" ");
    format!("({body})")
}

pub fn bool_atom(value: bool) -> &'static str {
    if value {
        "t"
    } else {
        "nil"
    }
}

pub fn observation_to_sexp(obs: &Observation) -> String {
    format!(
        "(:observation :cycle {} :global-score {} :local-score {} :signal {} :noise {} :chaos-risk {} \
         :rewrite-aggression {} :lorenz-bounded {} :lambdoma-ratio {} :rewrite-ready {} \
         :security-posture \"{}\" :security-events {} :route-success {} :route-latency {} \
         :cost-pressure {} :memory-pressure {} :graph-density {} :graph-interdisciplinary {} \
         :reward {} :stability {} :novelty {} :actor-load {} :actor-stalls {} :queue-depth {} \
         :error-pressure {} :prior-confidence {} :presentation-cleanliness {} \
         :presentation-verbosity {} :presentation-markdown-density {} \
         :presentation-symbolic-density {} :presentation-self-reference {} \
         :presentation-decor-density {} :presentation-user-affinity {})",
        obs.cycle,
        format_f64(obs.global_score),
        format_f64(obs.local_score),
        format_f64(obs.signal),
        format_f64(obs.noise),
        format_f64(obs.chaos_risk),
        format_f64(obs.rewrite_aggression),
        format_f64(obs.lorenz_bounded),
        format_f64(obs.lambdoma_ratio),
        bool_atom(obs.rewrite_ready),
        escape_string(&obs.security_posture),
        format_f64(obs.security_events),
        format_f64(obs.route_success),
        format_f64(obs.route_latency),
        format_f64(obs.cost_pressure),
        format_f64(obs.memory_pressure),
        format_f64(obs.graph_density),
        format_f64(obs.graph_interdisciplinary),
        format_f64(obs.reward),
        format_f64(obs.stability),
        format_f64(obs.novelty),
        format_f64(obs.actor_load),
        format_f64(obs.actor_stalls),
        format_f64(obs.queue_depth),
        format_f64(obs.error_pressure),
        format_f64(obs.prior_confidence),
        format_f64(obs.presentation_cleanliness),
        format_f64(obs.presentation_verbosity),
        format_f64(obs.presentation_markdown_density),
        format_f64(obs.presentation_symbolic_density),
        format_f64(obs.presentation_self_reference),
        format_f64(obs.presentation_decor_density),
        format_f64(obs.presentation_user_affinity),
    )
}

pub fn feedback_to_sexp(feedback: &Feedback) -> String {
    format!(
        "(:feedback :cycle {} :reward {} :stability {} :novelty {} :accepted {} :recall-hits {} \
         :user-affinity {} :cleanliness {} :applied-confidence {})",
        feedback.cycle,
        format_f64(feedback.reward),
        format_f64(feedback.stability),
        format_f64(feedback.novelty),
        bool_atom(feedback.accepted),
        feedback.recall_hits,
        format_f64(feedback.user_affinity),
        format_f64(feedback.cleanliness),
        format_f64(feedback.applied_confidence),
    )
}

pub fn projection_body_sexp(proj: &Projection) -> String {
    format!(
        ":cycle {} :confidence {} :stability {} :novelty {} :latent-energy {} :recall-strength {} \
         :harmony (:signal-bias {} :noise-bias {} :rewrite-signal-delta {} :rewrite-chaos-delta {} :aggression-bias {}) \
         :routing (:price-weight-delta {} :speed-weight-delta {} :success-weight-delta {} :reasoning-weight-delta {} :vitruvian-min-delta {}) \
         :memory (:recall-limit-delta {} :crystal-threshold-delta {}) \
         :security-shell (:dissonance-weight-delta {} :anomaly-threshold-delta {}) \
         :presentation (:verbosity-delta {} :markdown-density-delta {} :symbolic-density-delta {} \
         :self-reference-delta {} :decor-density-delta {})",
        proj.cycle,
        format_f64(proj.confidence),
        format_f64(proj.stability),
        format_f64(proj.novelty),
        format_f64(proj.latent_energy),
        format_f64(proj.recall_strength),
        format_f64(proj.harmony_signal_bias),
        format_f64(proj.harmony_noise_bias),
        format_f64(proj.rewrite_signal_delta),
        format_f64(proj.rewrite_chaos_delta),
        format_f64(proj.evolution_aggression_bias),
        format_f64(proj.routing_price_delta),
        format_f64(proj.routing_speed_delta),
        format_f64(proj.routing_success_delta),
        format_f64(proj.routing_reasoning_delta),
        format_f64(proj.routing_vitruvian_min_delta),
        proj.memory_recall_limit_delta,
        format_f64(proj.memory_crystal_threshold_delta),
        format_f64(proj.security_dissonance_delta),
        format_f64(proj.security_anomaly_delta),
        format_f64(proj.presentation_verbosity_delta),
        format_f64(proj.presentation_markdown_density_delta),
        format_f64(proj.presentation_symbolic_density_delta),
        format_f64(proj.presentation_self_reference_delta),
        format_f64(proj.presentation_decor_density_delta),
    )
}

#[allow(dead_code)]
pub fn projection_to_sexp(proj: &Projection) -> String {
    format!("(:signalograd-proposal {})", projection_body_sexp(proj))
}

pub fn status_sexp(state: &KernelState) -> String {
    format!(
        "(:cycle {} :actor-id {} :confidence {} :stability {} :novelty {} :latent-energy {} :recall-strength {} :memory-slots-used {} :checkpoint-digest \"{}\")",
        state.cycle,
        0, // actor-id now provided by runtime, not a global
        format_f64(state.last_projection.confidence),
        format_f64(state.last_projection.stability),
        format_f64(state.last_projection.novelty),
        format_f64(state.last_projection.latent_energy),
        format_f64(state.last_recall_strength),
        state.used_memory_slots(),
        digest_hex(state.checkpoint_digest),
    )
}

pub fn snapshot_sexp(state: &KernelState) -> String {
    format!(
        "(:signalograd-snapshot :cycle {} :lorenz (:x {} :y {} :z {}) :memory-slots-used {} \
         :recall-strength {} :last-feedback {} :last-projection ({}) :checkpoint-digest \"{}\")",
        state.cycle,
        format_f64(state.lorenz.x),
        format_f64(state.lorenz.y),
        format_f64(state.lorenz.z),
        state.used_memory_slots(),
        format_f64(state.last_recall_strength),
        feedback_to_sexp(&state.last_feedback),
        projection_body_sexp(&state.last_projection),
        digest_hex(state.checkpoint_digest),
    )
}
