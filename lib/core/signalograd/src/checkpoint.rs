use std::fs;
use std::path::{Path, PathBuf};

use crate::error::digest_hex;
use crate::error::{clamp, simple_hash};
use crate::format::{
    feedback_to_sexp, format_f64, observation_to_sexp, projection_body_sexp, vector_to_sexp,
};
use crate::model::{
    KernelState, LegacyKernelState, LorenzState, Projection, COMPONENT, HEAD_COUNT, INPUT_DIM,
    LATENT_DIM, MEMORY_SLOTS,
};
use crate::weights;
use crate::observation::{parse_feedback_sexp, parse_observation_sexp, parse_projection_sexp};
use harmonia_actor_protocol::sexp::{
    parse_fixed_array, parse_sexp, parse_vector_exact, plist_f64, plist_i64, plist_string,
    plist_value, plist_view,
};

pub fn default_state_root() -> String {
    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or(default)
}

pub fn default_state_path() -> String {
    format!("{}/signalograd.sexp", default_state_root())
}

pub fn signalograd_state_path() -> PathBuf {
    let default = default_state_path();
    let path =
        harmonia_config_store::get_own_or(COMPONENT, "state-path", &default).unwrap_or(default);
    PathBuf::from(path)
}

pub fn legacy_state_path() -> PathBuf {
    PathBuf::from(default_state_root()).join("signalograd.json")
}

pub fn load_state() -> Result<KernelState, String> {
    let path = signalograd_state_path();
    match fs::read_to_string(&path) {
        Ok(text) => parse_state_sexp(&text),
        Err(err) => {
            if let Ok(text) = fs::read_to_string(legacy_state_path()) {
                let migrated = import_legacy_state(&text)?;
                let _ = save_state(&migrated);
                return Ok(migrated);
            }
            Err(err.to_string())
        }
    }
}

pub fn save_state(state: &KernelState) -> Result<(), String> {
    write_state_to_path(state, &signalograd_state_path())
}

pub fn write_state_to_path(state: &KernelState, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = state_to_sexp(state);
    // Atomic write: temp file then rename to prevent corruption on crash.
    let tmp = path.with_extension("sexp.tmp");
    fs::write(&tmp, &body).map_err(|e| format!("signalograd tmp write: {e}"))?;
    fs::rename(&tmp, path).map_err(|e| format!("signalograd atomic rename: {e}"))?;
    Ok(())
}

pub fn restore_state_from_path(path: &Path) -> Result<KernelState, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_state_sexp(&text)
}

pub fn import_legacy_state(text: &str) -> Result<KernelState, String> {
    let legacy: LegacyKernelState =
        serde_json::from_str(text).map_err(|e| format!("legacy signalograd import failed: {e}"))?;
    let mut state = KernelState::new();
    state.cycle = legacy.cycle;
    for (index, value) in legacy.latent.iter().take(LATENT_DIM).enumerate() {
        state.latent[index] = clamp(*value, -1.0, 1.0);
    }
    state.last_projection = Projection {
        cycle: legacy.last_projection.cycle,
        confidence: legacy.last_projection.confidence,
        stability: legacy.last_projection.stability,
        novelty: legacy.last_projection.novelty,
        latent_energy: legacy.last_projection.latent_energy,
        recall_strength: 0.0,
        harmony_signal_bias: legacy.last_projection.harmony_signal_bias,
        harmony_noise_bias: legacy.last_projection.harmony_noise_bias,
        rewrite_signal_delta: legacy.last_projection.rewrite_signal_delta,
        rewrite_chaos_delta: legacy.last_projection.rewrite_chaos_delta,
        evolution_aggression_bias: legacy.last_projection.evolution_aggression_bias,
        logistic_r_delta: legacy.last_projection.logistic_r_delta,
        routing_price_delta: legacy.last_projection.routing_price_delta,
        routing_speed_delta: legacy.last_projection.routing_speed_delta,
        routing_success_delta: legacy.last_projection.routing_success_delta,
        routing_reasoning_delta: legacy.last_projection.routing_reasoning_delta,
        routing_vitruvian_min_delta: legacy.last_projection.routing_vitruvian_min_delta,
        memory_recall_limit_delta: legacy.last_projection.memory_recall_limit_delta,
        memory_crystal_threshold_delta: legacy.last_projection.memory_crystal_threshold_delta,
        security_dissonance_delta: legacy.last_projection.security_dissonance_delta,
        security_anomaly_delta: legacy.last_projection.security_anomaly_delta,
        presentation_verbosity_delta: legacy.last_projection.presentation_verbosity_delta,
        presentation_markdown_density_delta: legacy
            .last_projection
            .presentation_markdown_density_delta,
        presentation_symbolic_density_delta: legacy
            .last_projection
            .presentation_symbolic_density_delta,
        presentation_self_reference_delta: legacy.last_projection.presentation_self_reference_delta,
        presentation_decor_density_delta: legacy.last_projection.presentation_decor_density_delta,
    };
    Ok(state)
}

pub fn state_to_sexp(state: &KernelState) -> String {
    let flat_readout = state
        .readout_weights
        .iter()
        .flat_map(|row| row.iter().copied())
        .collect::<Vec<_>>();
    let flat_slots = state
        .memory_slots
        .iter()
        .flat_map(|slot| slot.iter().copied())
        .collect::<Vec<_>>();

    format!(
        "(:signalograd-state :cycle {} :lorenz (:x {} :y {} :z {}) :latent {} \
         :input-matrix {} :recurrent-matrix {} :readout-matrix {} :memory-slots {} \
         :memory-strengths {} :memory-usage {} :dynamic-weights {} :last-feedback {} :last-observation {} \
         :last-projection ({}) :last-recall-strength {} :checkpoint-digest \"{}\")",
        state.cycle,
        format_f64(state.lorenz.x),
        format_f64(state.lorenz.y),
        format_f64(state.lorenz.z),
        vector_to_sexp(&state.latent),
        vector_to_sexp(&state.input_matrix),
        vector_to_sexp(&state.recurrent_matrix),
        vector_to_sexp(&flat_readout),
        vector_to_sexp(&flat_slots),
        vector_to_sexp(&state.memory_strengths),
        vector_to_sexp(&state.memory_usage),
        vector_to_sexp(&state.dynamic_weights),
        feedback_to_sexp(&state.last_feedback),
        observation_to_sexp(&state.last_observation),
        projection_body_sexp(&state.last_projection),
        format_f64(state.last_recall_strength),
        digest_hex(state.checkpoint_digest),
    )
}

pub fn parse_state_sexp(raw: &str) -> Result<KernelState, String> {
    let sexp = parse_sexp(raw)?;
    let items = plist_view(&sexp)?;
    let mut state = KernelState::new();
    state.cycle = plist_i64(items, "cycle").unwrap_or(0);
    if let Some(lorenz) = plist_value(items, "lorenz") {
        let lorenz_items = plist_view(lorenz)?;
        state.lorenz = LorenzState {
            x: plist_f64(lorenz_items, "x").unwrap_or(0.1),
            y: plist_f64(lorenz_items, "y").unwrap_or(0.0),
            z: plist_f64(lorenz_items, "z").unwrap_or(0.0),
        };
    }
    state.latent = parse_fixed_array::<LATENT_DIM>(plist_value(items, "latent"), "latent")?;
    state.input_matrix = parse_vector_exact(
        plist_value(items, "input-matrix"),
        LATENT_DIM * INPUT_DIM,
        "input-matrix",
    )?;
    state.recurrent_matrix = parse_vector_exact(
        plist_value(items, "recurrent-matrix"),
        LATENT_DIM * LATENT_DIM,
        "recurrent-matrix",
    )?;
    let flat_readout = parse_vector_exact(
        plist_value(items, "readout-matrix"),
        HEAD_COUNT * LATENT_DIM,
        "readout-matrix",
    )?;
    state.readout_weights = flat_readout
        .chunks(LATENT_DIM)
        .map(|chunk| chunk.to_vec())
        .collect();
    let flat_slots = parse_vector_exact(
        plist_value(items, "memory-slots"),
        MEMORY_SLOTS * LATENT_DIM,
        "memory-slots",
    )?;
    state.memory_slots = flat_slots
        .chunks(LATENT_DIM)
        .map(|chunk| chunk.to_vec())
        .collect();
    state.memory_strengths = parse_vector_exact(
        plist_value(items, "memory-strengths"),
        MEMORY_SLOTS,
        "memory-strengths",
    )?;
    state.memory_usage = parse_vector_exact(
        plist_value(items, "memory-usage"),
        MEMORY_SLOTS,
        "memory-usage",
    )?;
    // Parse dynamic weights with fallback to initial conditions for old checkpoints.
    if let Some(dw_sexp) = plist_value(items, "dynamic-weights") {
        state.dynamic_weights = parse_vector_exact(
            Some(dw_sexp),
            weights::DYNAMIC_WEIGHT_COUNT,
            "dynamic-weights",
        )?;
    }
    // else: keep the initialized defaults from KernelState::new()
    if let Some(value) = plist_value(items, "last-feedback") {
        state.last_feedback = parse_feedback_sexp(value)?;
    }
    if let Some(value) = plist_value(items, "last-observation") {
        state.last_observation = parse_observation_sexp(value)?;
    }
    if let Some(value) = plist_value(items, "last-projection") {
        state.last_projection = parse_projection_sexp(value)?;
    }
    state.last_recall_strength = plist_f64(items, "last-recall-strength").unwrap_or(0.0);
    state.checkpoint_digest = plist_string(items, "checkpoint-digest")
        .and_then(|value| u64::from_str_radix(value.trim(), 16).ok())
        .unwrap_or_else(|| simple_hash(raw));
    Ok(state)
}
