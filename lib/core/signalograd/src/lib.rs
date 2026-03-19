use serde::Deserialize;
use std::ffi::{CStr, CString};
use std::fs;
use std::os::raw::c_char;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};

const LATENT_DIM: usize = 32;
const INPUT_DIM: usize = 25;
const MEMORY_SLOTS: usize = 32;
const HEAD_COUNT: usize = 5;
const PHI: f64 = 1.618_033_988_749_895;
const FEIGENBAUM_DELTA: f64 = 4.669_201_609_102_99;
const FEIGENBAUM_ALPHA: f64 = 2.502_907_875_095_892_6;
const VERSION: &[u8] = b"harmonia-signalograd/0.2.0\0";
const COMPONENT: &str = "signalograd-core";

static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();
static STATE: OnceLock<Mutex<KernelState>> = OnceLock::new();
static ACTOR_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct Observation {
    cycle: i64,
    global_score: f64,
    local_score: f64,
    signal: f64,
    noise: f64,
    chaos_risk: f64,
    rewrite_aggression: f64,
    lorenz_bounded: f64,
    lambdoma_ratio: f64,
    rewrite_ready: bool,
    security_posture: String,
    security_events: f64,
    route_success: f64,
    route_latency: f64,
    cost_pressure: f64,
    memory_pressure: f64,
    graph_density: f64,
    graph_interdisciplinary: f64,
    reward: f64,
    stability: f64,
    novelty: f64,
    actor_load: f64,
    actor_stalls: f64,
    queue_depth: f64,
    error_pressure: f64,
    supervision: f64,
    prior_confidence: f64,
    presentation_cleanliness: f64,
    presentation_verbosity: f64,
    presentation_markdown_density: f64,
    presentation_symbolic_density: f64,
    presentation_self_reference: f64,
    presentation_decor_density: f64,
    presentation_user_affinity: f64,
}

#[derive(Debug, Clone, Default)]
struct Feedback {
    cycle: i64,
    reward: f64,
    stability: f64,
    novelty: f64,
    accepted: bool,
    recall_hits: i64,
    user_affinity: f64,
    cleanliness: f64,
    applied_confidence: f64,
}

#[derive(Debug, Clone, Default)]
struct LorenzState {
    x: f64,
    y: f64,
    z: f64,
}

#[derive(Clone, Debug, Default)]
struct Projection {
    cycle: i64,
    confidence: f64,
    stability: f64,
    novelty: f64,
    latent_energy: f64,
    recall_strength: f64,
    harmony_signal_bias: f64,
    harmony_noise_bias: f64,
    rewrite_signal_delta: f64,
    rewrite_chaos_delta: f64,
    evolution_aggression_bias: f64,
    routing_price_delta: f64,
    routing_speed_delta: f64,
    routing_success_delta: f64,
    routing_reasoning_delta: f64,
    routing_vitruvian_min_delta: f64,
    memory_recall_limit_delta: i32,
    memory_crystal_threshold_delta: f64,
    security_dissonance_delta: f64,
    security_anomaly_delta: f64,
    presentation_verbosity_delta: f64,
    presentation_markdown_density_delta: f64,
    presentation_symbolic_density_delta: f64,
    presentation_self_reference_delta: f64,
    presentation_decor_density_delta: f64,
}

#[derive(Clone, Debug)]
struct KernelState {
    cycle: i64,
    lorenz: LorenzState,
    latent: [f64; LATENT_DIM],
    input_matrix: Vec<f64>,
    recurrent_matrix: Vec<f64>,
    readout_weights: Vec<Vec<f64>>,
    memory_slots: Vec<Vec<f64>>,
    memory_strengths: Vec<f64>,
    memory_usage: Vec<f64>,
    last_projection: Projection,
    last_feedback: Feedback,
    last_observation: Observation,
    last_recall_strength: f64,
    checkpoint_digest: u64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct LegacyKernelState {
    cycle: i64,
    latent: Vec<f64>,
    last_projection: LegacyProjection,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct LegacyProjection {
    cycle: i64,
    confidence: f64,
    stability: f64,
    novelty: f64,
    latent_energy: f64,
    harmony_signal_bias: f64,
    harmony_noise_bias: f64,
    rewrite_signal_delta: f64,
    rewrite_chaos_delta: f64,
    evolution_aggression_bias: f64,
    routing_price_delta: f64,
    routing_speed_delta: f64,
    routing_success_delta: f64,
    routing_reasoning_delta: f64,
    routing_vitruvian_min_delta: f64,
    memory_recall_limit_delta: i32,
    memory_crystal_threshold_delta: f64,
    security_dissonance_delta: f64,
    security_anomaly_delta: f64,
    presentation_verbosity_delta: f64,
    presentation_markdown_density_delta: f64,
    presentation_symbolic_density_delta: f64,
    presentation_self_reference_delta: f64,
    presentation_decor_density_delta: f64,
}

#[derive(Debug, Clone)]
enum Sexp {
    List(Vec<Sexp>),
    Atom(String),
    String(String),
}

impl KernelState {
    fn new() -> Self {
        let mut input_matrix = vec![0.0; LATENT_DIM * INPUT_DIM];
        let mut recurrent_matrix = vec![0.0; LATENT_DIM * LATENT_DIM];
        let mut readout_weights = vec![vec![0.0; LATENT_DIM]; HEAD_COUNT];

        for row in 0..LATENT_DIM {
            for col in 0..INPUT_DIM {
                input_matrix[row * INPUT_DIM + col] = seeded_weight(row, col, 0.19);
            }
            for col in 0..LATENT_DIM {
                recurrent_matrix[row * LATENT_DIM + col] = if (row + col * 2) % 7 == 0 {
                    seeded_weight(row + 13, col + 17, 0.11)
                } else {
                    0.0
                };
            }
            for head in 0..HEAD_COUNT {
                readout_weights[head][row] = seeded_weight(head + 31, row + 37, 0.08);
            }
        }

        Self {
            cycle: 0,
            lorenz: LorenzState {
                x: 0.1,
                y: 0.0,
                z: 0.0,
            },
            latent: [0.0; LATENT_DIM],
            input_matrix,
            recurrent_matrix,
            readout_weights,
            memory_slots: vec![vec![0.0; LATENT_DIM]; MEMORY_SLOTS],
            memory_strengths: vec![0.0; MEMORY_SLOTS],
            memory_usage: vec![0.0; MEMORY_SLOTS],
            last_projection: Projection::default(),
            last_feedback: Feedback::default(),
            last_observation: Observation::default(),
            last_recall_strength: 0.0,
            checkpoint_digest: 0,
        }
    }

    fn used_memory_slots(&self) -> usize {
        self.memory_strengths
            .iter()
            .filter(|strength| **strength > 0.001)
            .count()
    }
}

fn last_error() -> &'static Mutex<String> {
    LAST_ERROR.get_or_init(|| Mutex::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error().lock() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error().lock() {
        slot.clear();
    }
}

fn last_error_message() -> String {
    last_error()
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_else(|_| "signalograd error lock poisoned".to_string())
}

fn state() -> &'static Mutex<KernelState> {
    STATE.get_or_init(|| Mutex::new(load_state().unwrap_or_else(|_| KernelState::new())))
}

fn seeded_weight(a: usize, b: usize, scale: f64) -> f64 {
    let x = ((a + 1) as f64 * PHI + (b + 1) as f64 / FEIGENBAUM_DELTA).sin()
        + ((a + 1) as f64 / FEIGENBAUM_ALPHA + (b + 1) as f64 * 0.5).cos();
    clamp(x * 0.5 * scale, -scale, scale)
}

fn clamp(x: f64, lo: f64, hi: f64) -> f64 {
    x.max(lo).min(hi)
}

fn simple_hash(input: &str) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in input.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn digest_hex(digest: u64) -> String {
    format!("{digest:016x}")
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

fn ensure_actor_registered() {
    // Actor registration is now handled by the runtime IPC system.
}

fn default_state_root() -> String {
    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or(default)
}

fn default_state_path() -> String {
    format!("{}/signalograd.sexp", default_state_root())
}

fn signalograd_state_path() -> PathBuf {
    let default = default_state_path();
    let path =
        harmonia_config_store::get_own_or(COMPONENT, "state-path", &default).unwrap_or(default);
    PathBuf::from(path)
}

fn legacy_state_path() -> PathBuf {
    PathBuf::from(default_state_root()).join("signalograd.json")
}

fn load_state() -> Result<KernelState, String> {
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

fn save_state(state: &KernelState) -> Result<(), String> {
    write_state_to_path(state, &signalograd_state_path())
}

fn write_state_to_path(state: &KernelState, path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let body = state_to_sexp(state);
    fs::write(path, &body).map_err(|e| e.to_string())?;
    Ok(())
}

fn restore_state_from_path(path: &Path) -> Result<KernelState, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_state_sexp(&text)
}

fn import_legacy_state(text: &str) -> Result<KernelState, String> {
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

fn escape_string(input: &str) -> String {
    input
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\t', "\\t")
}

fn format_f64(value: f64) -> String {
    format!("{value:.9}")
}

fn vector_to_sexp(values: &[f64]) -> String {
    let body = values
        .iter()
        .map(|value| format_f64(*value))
        .collect::<Vec<_>>()
        .join(" ");
    format!("({body})")
}

fn bool_atom(value: bool) -> &'static str {
    if value {
        "t"
    } else {
        "nil"
    }
}

fn observation_to_sexp(obs: &Observation) -> String {
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

fn feedback_to_sexp(feedback: &Feedback) -> String {
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

fn projection_body_sexp(proj: &Projection) -> String {
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
fn projection_to_sexp(proj: &Projection) -> String {
    format!("(:signalograd-proposal {})", projection_body_sexp(proj))
}

fn state_to_sexp(state: &KernelState) -> String {
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
         :memory-strengths {} :memory-usage {} :last-feedback {} :last-observation {} \
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
        feedback_to_sexp(&state.last_feedback),
        observation_to_sexp(&state.last_observation),
        projection_body_sexp(&state.last_projection),
        format_f64(state.last_recall_strength),
        digest_hex(state.checkpoint_digest),
    )
}

fn status_sexp(state: &KernelState) -> String {
    format!(
        "(:cycle {} :actor-id {} :confidence {} :stability {} :novelty {} :latent-energy {} :recall-strength {} :memory-slots-used {} :checkpoint-digest \"{}\")",
        state.cycle,
        ACTOR_ID.load(Ordering::SeqCst),
        format_f64(state.last_projection.confidence),
        format_f64(state.last_projection.stability),
        format_f64(state.last_projection.novelty),
        format_f64(state.last_projection.latent_energy),
        format_f64(state.last_recall_strength),
        state.used_memory_slots(),
        digest_hex(state.checkpoint_digest),
    )
}

fn snapshot_sexp(state: &KernelState) -> String {
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

fn posture_scalar(posture: &str) -> f64 {
    match posture {
        "alert" => 1.0,
        "elevated" => 0.6,
        "nominal" => 0.1,
        _ => 0.25,
    }
}

fn observation_vector(obs: &Observation) -> [f64; INPUT_DIM] {
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
    ]
}

fn dot(weights: &[f64], latent: &[f64; LATENT_DIM]) -> f64 {
    weights
        .iter()
        .zip(latent.iter())
        .map(|(w, l)| *w * *l)
        .sum::<f64>()
        / LATENT_DIM as f64
}

fn normalize_latent(latent: &mut [f64; LATENT_DIM]) {
    let mean = latent.iter().sum::<f64>() / LATENT_DIM as f64;
    let energy = latent.iter().map(|value| value * value).sum::<f64>().sqrt();
    let scale = if energy > 1.0 { 1.0 / energy } else { 1.0 };
    for value in latent.iter_mut() {
        *value = clamp((*value - 0.12 * mean) * scale, -1.0, 1.0);
    }
}

fn update_lorenz(lorenz: &mut LorenzState, obs: &Observation) {
    let sigma = 10.0 + 2.0 * obs.signal - obs.noise;
    let rho = 28.0 + 5.0 * (obs.global_score - obs.chaos_risk) + 2.0 * obs.route_success;
    let beta = 8.0 / 3.0 + 0.08 * obs.memory_pressure;
    let dt = clamp(
        0.008 + 0.004 * obs.stability + 0.002 * obs.novelty,
        0.004,
        0.02,
    );
    let dx = sigma * (lorenz.y - lorenz.x)
        + 0.35 * (obs.signal - obs.noise)
        + 0.08 * (obs.actor_load - obs.actor_stalls);
    let dy = lorenz.x * (rho - lorenz.z) - lorenz.y + 0.22 * obs.route_success
        - 0.12 * obs.cost_pressure;
    let dz = lorenz.x * lorenz.y - beta * lorenz.z + 0.18 * obs.graph_density;
    lorenz.x = clamp(lorenz.x + dt * dx, -40.0, 40.0);
    lorenz.y = clamp(lorenz.y + dt * dy, -50.0, 50.0);
    lorenz.z = clamp(lorenz.z + dt * dz, 0.0, 60.0);
}

fn lorenz_energy(lorenz: &LorenzState) -> f64 {
    clamp(
        (lorenz.x * lorenz.x + lorenz.y * lorenz.y + lorenz.z * lorenz.z).sqrt() / 40.0,
        0.0,
        1.0,
    )
}

fn lorenz_basis(lorenz: &LorenzState, row: usize) -> f64 {
    let phase = ((row + 1) as f64 * PHI).sin();
    let x = lorenz.x * phase;
    let y = lorenz.y * (phase * 0.7).cos();
    let z = lorenz.z * (phase * 0.3).sin();
    0.018 * (x + y + z)
}

fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
    let mut dot = 0.0;
    let mut norm_a = 0.0;
    let mut norm_b = 0.0;
    for (left, right) in a.iter().zip(b.iter()) {
        dot += left * right;
        norm_a += left * left;
        norm_b += right * right;
    }
    if norm_a <= f64::EPSILON || norm_b <= f64::EPSILON {
        0.0
    } else {
        clamp(dot / (norm_a.sqrt() * norm_b.sqrt()), -1.0, 1.0)
    }
}

fn hopfield_recall(state: &KernelState) -> ([f64; LATENT_DIM], f64, i64) {
    let latent = state.latent.iter().copied().collect::<Vec<_>>();
    let mut best_index = None;
    let mut best_score = 0.0;

    for index in 0..MEMORY_SLOTS {
        let strength = state.memory_strengths[index];
        if strength <= 0.001 {
            continue;
        }
        let similarity = cosine_similarity(&latent, &state.memory_slots[index]);
        let score = similarity * (0.65 + 0.35 * strength);
        if score > best_score {
            best_score = score;
            best_index = Some(index);
        }
    }

    let mut recalled = [0.0; LATENT_DIM];
    if let Some(index) = best_index {
        let strength = clamp(best_score, 0.0, 1.0);
        for (slot, out) in state.memory_slots[index].iter().zip(recalled.iter_mut()) {
            *out = *slot * strength;
        }
        (recalled, strength, if strength > 0.12 { 1 } else { 0 })
    } else {
        (recalled, 0.0, 0)
    }
}

fn update_local_weights(
    weights: &mut [f64],
    latent: &[f64; LATENT_DIM],
    target: f64,
    eta: f64,
) -> f64 {
    let prediction = clamp(dot(weights, latent), -1.0, 1.0);
    let error = target - prediction;
    for (index, weight) in weights.iter_mut().enumerate() {
        let hebbian = error * latent[index];
        let oja = prediction * latent[index] * latent[index] * 0.03;
        *weight = clamp(*weight * 0.998 + eta * hebbian - eta * oja, -2.5, 2.5);
    }
    prediction
}

fn head_targets(obs: &Observation, recall_strength: f64, lorenz_energy: f64) -> [f64; HEAD_COUNT] {
    let harmony = clamp(
        obs.signal - obs.noise + 0.25 * obs.reward + 0.15 * recall_strength + 0.10 * lorenz_energy
            - 0.20 * obs.chaos_risk
            + 0.12 * obs.presentation_cleanliness
            + 0.08 * obs.presentation_user_affinity
            - 0.10 * obs.presentation_symbolic_density
            - 0.10 * obs.presentation_decor_density,
        -1.0,
        1.0,
    );
    let routing = clamp(
        obs.route_success - 0.55 * obs.cost_pressure - 0.35 * obs.route_latency
            + 0.20 * obs.prior_confidence
            + 0.10 * recall_strength,
        -1.0,
        1.0,
    );
    let memory = clamp(
        (1.0 - obs.memory_pressure) * 0.55
            + 0.20 * obs.stability
            + 0.15 * recall_strength
            + 0.10 * obs.presentation_user_affinity
            - 0.10 * obs.presentation_verbosity,
        -1.0,
        1.0,
    );
    let evolution = clamp(
        obs.signal - obs.chaos_risk
            + if obs.rewrite_ready { 0.25 } else { -0.10 }
            + 0.15 * obs.stability
            + 0.10 * obs.presentation_cleanliness,
        -1.0,
        1.0,
    );
    let security = clamp(
        1.0 - posture_scalar(obs.security_posture.trim())
            - 0.20 * obs.noise
            - 0.15 * obs.error_pressure
            - 0.08 * obs.presentation_decor_density,
        -1.0,
        1.0,
    );
    [harmony, routing, memory, evolution, security]
}

fn build_projection(
    state: &KernelState,
    obs: &Observation,
    predictions: [f64; HEAD_COUNT],
    recall_strength: f64,
) -> Projection {
    let harmony_head = predictions[0];
    let routing_head = predictions[1];
    let memory_head = predictions[2];
    let evolution_head = predictions[3];
    let security_head = predictions[4];
    let latent_energy = state
        .latent
        .iter()
        .map(|value| value * value)
        .sum::<f64>()
        .sqrt()
        / (LATENT_DIM as f64).sqrt();
    let affinity = obs.presentation_user_affinity * 2.0 - 1.0;
    let cleanliness = obs.presentation_cleanliness * 2.0 - 1.0;
    let confidence = clamp(
        0.35 * obs.stability
            + 0.20 * (1.0 - obs.chaos_risk)
            + 0.15 * (1.0 - obs.noise)
            + 0.15 * recall_strength
            + 0.10 * lorenz_energy(&state.lorenz)
            + 0.05 * obs.presentation_cleanliness,
        0.0,
        1.0,
    );

    Projection {
        cycle: obs.cycle,
        confidence,
        stability: clamp(obs.stability, 0.0, 1.0),
        novelty: clamp(obs.novelty, 0.0, 1.0),
        latent_energy,
        recall_strength,
        harmony_signal_bias: clamp(
            (0.80 * harmony_head + 0.20 * recall_strength) * 0.06,
            -0.06,
            0.06,
        ),
        harmony_noise_bias: clamp(
            (-0.70 * harmony_head - 0.30 * recall_strength) * 0.04,
            -0.04,
            0.04,
        ),
        rewrite_signal_delta: clamp(
            (0.75 * harmony_head + 0.25 * recall_strength) * 0.05,
            -0.05,
            0.05,
        ),
        rewrite_chaos_delta: clamp(
            (-0.70 * evolution_head - 0.30 * recall_strength) * 0.04,
            -0.04,
            0.04,
        ),
        evolution_aggression_bias: clamp(
            (0.75 * evolution_head + 0.25 * lorenz_energy(&state.lorenz)) * 0.08,
            -0.08,
            0.08,
        ),
        routing_price_delta: clamp(
            (-0.70 * routing_head - 0.30 * obs.cost_pressure) * 0.07,
            -0.07,
            0.07,
        ),
        routing_speed_delta: clamp(
            (0.75 * routing_head + 0.25 * recall_strength) * 0.07,
            -0.07,
            0.07,
        ),
        routing_success_delta: clamp(
            (0.65 * routing_head + 0.35 * obs.route_success) * 0.05,
            -0.05,
            0.05,
        ),
        routing_reasoning_delta: clamp(
            0.6 * routing_head + 0.25 * harmony_head + 0.15 * recall_strength,
            -0.06,
            0.06,
        ),
        routing_vitruvian_min_delta: clamp(
            (0.7 * harmony_head + 0.3 * recall_strength) * 0.04,
            -0.04,
            0.04,
        ),
        memory_recall_limit_delta: clamp(memory_head * 2.0 + recall_strength * 1.5, -2.0, 2.0)
            .round() as i32,
        memory_crystal_threshold_delta: clamp(
            (0.75 * memory_head + 0.25 * recall_strength) * 0.05,
            -0.05,
            0.05,
        ),
        security_dissonance_delta: clamp(
            (-0.80 * security_head - 0.20 * obs.error_pressure) * 0.03,
            -0.03,
            0.03,
        ),
        security_anomaly_delta: clamp(
            (0.70 * security_head - 0.30 * obs.error_pressure) * 0.25,
            -0.25,
            0.25,
        ),
        presentation_verbosity_delta: clamp(
            (0.50 * memory_head - 0.45 * obs.presentation_verbosity
                + 0.20 * affinity
                + 0.15 * cleanliness)
                * 0.22,
            -0.22,
            0.22,
        ),
        presentation_markdown_density_delta: clamp(
            (0.40 * memory_head - 0.50 * obs.presentation_markdown_density + 0.20 * affinity)
                * 0.18,
            -0.18,
            0.18,
        ),
        presentation_symbolic_density_delta: clamp(
            (-0.65 * obs.presentation_symbolic_density
                + 0.20 * harmony_head
                + 0.15 * recall_strength
                + 0.20 * cleanliness)
                * 0.22,
            -0.22,
            0.22,
        ),
        presentation_self_reference_delta: clamp(
            (-0.70 * obs.presentation_self_reference
                + 0.15 * harmony_head
                + 0.15 * affinity
                + 0.10 * cleanliness)
                * 0.22,
            -0.22,
            0.22,
        ),
        presentation_decor_density_delta: clamp(
            (-0.80 * obs.presentation_decor_density - 0.35 * (1.0 - obs.presentation_cleanliness)
                + 0.15 * harmony_head)
                * 0.25,
            -0.25,
            0.25,
        ),
    }
}

fn step_kernel(state: &mut KernelState, obs: &Observation) -> Projection {
    update_lorenz(&mut state.lorenz, obs);
    let input = observation_vector(obs);
    let (recalled, recall_strength, _) = hopfield_recall(state);
    let mut next_latent = [0.0; LATENT_DIM];

    for row in 0..LATENT_DIM {
        let mut acc = 0.42 * state.latent[row];
        for (col, value) in input.iter().enumerate() {
            acc += state.input_matrix[row * INPUT_DIM + col] * *value;
        }
        for col in 0..LATENT_DIM {
            acc += state.recurrent_matrix[row * LATENT_DIM + col] * state.latent[col];
        }
        acc += 0.14 * recalled[row];
        acc += lorenz_basis(&state.lorenz, row);
        next_latent[row] = clamp(acc.tanh(), -1.0, 1.0);
    }

    normalize_latent(&mut next_latent);
    state.latent = next_latent;
    state.cycle = obs.cycle;
    state.last_observation = obs.clone();
    state.last_recall_strength = recall_strength;

    for usage in state.memory_usage.iter_mut() {
        *usage *= 0.995;
    }

    let targets = head_targets(obs, recall_strength, lorenz_energy(&state.lorenz));
    let mut predictions = [0.0; HEAD_COUNT];
    for (index, head) in state.readout_weights.iter_mut().enumerate() {
        predictions[index] = update_local_weights(head, &state.latent, targets[index], 0.045);
    }

    let projection = build_projection(state, obs, predictions, recall_strength);
    state.last_projection = projection.clone();
    projection
}

fn feedback_targets(
    obs: &Observation,
    feedback: &Feedback,
    recall_strength: f64,
) -> [f64; HEAD_COUNT] {
    let outcome = clamp(
        0.45 * feedback.reward
            + 0.20 * feedback.stability
            + 0.15 * feedback.novelty
            + 0.10 * feedback.user_affinity
            + 0.10 * feedback.cleanliness,
        0.0,
        1.0,
    );
    let centered = outcome * 2.0 - 1.0;
    [
        clamp(
            centered - 0.25 * obs.noise - 0.15 * obs.presentation_symbolic_density
                + 0.15 * feedback.cleanliness
                + 0.10 * recall_strength,
            -1.0,
            1.0,
        ),
        clamp(
            centered - 0.25 * obs.cost_pressure - 0.20 * obs.route_latency
                + 0.10 * feedback.applied_confidence
                + 0.10 * feedback.user_affinity,
            -1.0,
            1.0,
        ),
        clamp(
            centered - 0.20 * obs.memory_pressure
                + 0.20 * recall_strength
                + 0.15 * feedback.user_affinity,
            -1.0,
            1.0,
        ),
        clamp(
            centered - 0.25 * obs.chaos_risk
                + if feedback.accepted { 0.15 } else { -0.05 }
                + 0.10 * feedback.cleanliness,
            -1.0,
            1.0,
        ),
        clamp(
            centered
                - 0.20 * obs.error_pressure
                - 0.15 * obs.noise
                - 0.10 * obs.presentation_decor_density
                + 0.10 * feedback.cleanliness,
            -1.0,
            1.0,
        ),
    ]
}

fn remember_state(state: &mut KernelState, feedback: &Feedback) {
    let latent = state.latent.iter().copied().collect::<Vec<_>>();
    let mut best_index = None;
    let mut best_similarity = 0.0;

    for index in 0..MEMORY_SLOTS {
        let similarity = cosine_similarity(&latent, &state.memory_slots[index]);
        if similarity > best_similarity {
            best_similarity = similarity;
            best_index = Some(index);
        }
    }

    let slot_index = if best_similarity > 0.92 {
        best_index.unwrap_or(0)
    } else {
        let mut lowest = 0usize;
        let mut lowest_score = f64::INFINITY;
        for index in 0..MEMORY_SLOTS {
            let score = 0.7 * state.memory_strengths[index] + 0.3 * state.memory_usage[index];
            if score < lowest_score {
                lowest_score = score;
                lowest = index;
            }
        }
        lowest
    };

    for (target, value) in state.memory_slots[slot_index]
        .iter_mut()
        .zip(state.latent.iter())
    {
        *target = clamp(0.72 * *target + 0.28 * *value, -1.0, 1.0);
    }

    state.memory_strengths[slot_index] = clamp(
        0.78 * state.memory_strengths[slot_index]
            + 0.22
                * (0.45 * feedback.reward
                    + 0.25 * feedback.stability
                    + 0.15 * feedback.user_affinity
                    + 0.15 * feedback.cleanliness),
        0.0,
        1.0,
    );
    state.memory_usage[slot_index] = clamp(state.memory_usage[slot_index] + 1.0, 0.0, 1000.0);
}

fn apply_feedback(state: &mut KernelState, feedback: &Feedback) {
    let targets = feedback_targets(
        &state.last_observation,
        feedback,
        state.last_recall_strength,
    );
    let eta = clamp(
        0.025
            + 0.035 * feedback.reward
            + 0.015 * feedback.stability
            + 0.010 * feedback.user_affinity
            + 0.005 * feedback.cleanliness,
        0.02,
        0.09,
    );
    for (index, head) in state.readout_weights.iter_mut().enumerate() {
        let _ = update_local_weights(head, &state.latent, targets[index], eta);
    }

    if feedback.accepted
        || (feedback.reward > 0.58
            && feedback.stability > 0.55
            && feedback.user_affinity > 0.35
            && feedback.cleanliness > 0.55)
    {
        remember_state(state, feedback);
    } else {
        for strength in state.memory_strengths.iter_mut() {
            *strength = clamp(*strength * 0.999, 0.0, 1.0);
        }
    }

    state.last_feedback = feedback.clone();
}

fn post_projection_signal(_projection: &Projection) {
    // Actor mailbox posting is now handled by the runtime IPC system.
}

fn parse_observation(raw: &str) -> Result<Observation, String> {
    if raw.trim_start().starts_with('{') {
        return serde_json::from_str(raw)
            .map_err(|e| format!("invalid signalograd observation: {e}"));
    }
    let sexp = parse_sexp(raw)?;
    parse_observation_sexp(&sexp)
}

fn parse_feedback(raw: &str) -> Result<Feedback, String> {
    let sexp = parse_sexp(raw)?;
    parse_feedback_sexp(&sexp)
}

fn parse_observation_sexp(sexp: &Sexp) -> Result<Observation, String> {
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
        presentation_cleanliness: plist_f64(items, "presentation-cleanliness").unwrap_or(1.0),
        presentation_verbosity: plist_f64(items, "presentation-verbosity").unwrap_or(0.0),
        presentation_markdown_density: plist_f64(items, "presentation-markdown-density")
            .unwrap_or(0.0),
        presentation_symbolic_density: plist_f64(items, "presentation-symbolic-density")
            .unwrap_or(0.0),
        presentation_self_reference: plist_f64(items, "presentation-self-reference").unwrap_or(0.0),
        presentation_decor_density: plist_f64(items, "presentation-decor-density").unwrap_or(0.0),
        presentation_user_affinity: plist_f64(items, "presentation-user-affinity").unwrap_or(0.5),
    })
}

fn parse_feedback_sexp(sexp: &Sexp) -> Result<Feedback, String> {
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

fn parse_projection_sexp(sexp: &Sexp) -> Result<Projection, String> {
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

fn parse_state_sexp(raw: &str) -> Result<KernelState, String> {
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

fn parse_fixed_array<const N: usize>(sexp: Option<&Sexp>, label: &str) -> Result<[f64; N], String> {
    let values = parse_vector_exact(sexp, N, label)?;
    let mut output = [0.0; N];
    for (slot, value) in output.iter_mut().zip(values.iter()) {
        *slot = *value;
    }
    Ok(output)
}

fn parse_vector_exact(
    sexp: Option<&Sexp>,
    expected: usize,
    label: &str,
) -> Result<Vec<f64>, String> {
    let values = parse_number_list(sexp.ok_or_else(|| format!("missing {label}"))?)?;
    if values.len() != expected {
        return Err(format!(
            "invalid {label}: expected {expected} values, got {}",
            values.len()
        ));
    }
    Ok(values)
}

fn parse_number_list(sexp: &Sexp) -> Result<Vec<f64>, String> {
    match sexp {
        Sexp::List(items) => items
            .iter()
            .map(|item| sexp_to_f64(item).ok_or_else(|| "expected numeric atom".to_string()))
            .collect(),
        _ => Err("expected list".to_string()),
    }
}

fn parse_sexp(raw: &str) -> Result<Sexp, String> {
    let mut parser = Parser::new(raw);
    let sexp = parser.parse_expr()?;
    parser.skip_ws();
    if parser.peek().is_some() {
        return Err("unexpected trailing content".to_string());
    }
    Ok(sexp)
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    _raw: &'a str,
}

impl<'a> Parser<'a> {
    fn new(raw: &'a str) -> Self {
        Self {
            chars: raw.chars().collect(),
            index: 0,
            _raw: raw,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek()?;
        self.index += 1;
        Some(ch)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
            self.index += 1;
        }
    }

    fn parse_expr(&mut self) -> Result<Sexp, String> {
        self.skip_ws();
        match self.peek() {
            Some('(') => self.parse_list(),
            Some('"') => self.parse_string(),
            Some(_) => self.parse_atom(),
            None => Err("unexpected end of input".to_string()),
        }
    }

    fn parse_list(&mut self) -> Result<Sexp, String> {
        self.bump();
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(')') => {
                    self.bump();
                    return Ok(Sexp::List(items));
                }
                Some(_) => items.push(self.parse_expr()?),
                None => return Err("unterminated list".to_string()),
            }
        }
    }

    fn parse_string(&mut self) -> Result<Sexp, String> {
        self.bump();
        let mut out = String::new();
        loop {
            match self.bump() {
                Some('"') => return Ok(Sexp::String(out)),
                Some('\\') => match self.bump() {
                    Some('"') => out.push('"'),
                    Some('\\') => out.push('\\'),
                    Some('n') => out.push('\n'),
                    Some('t') => out.push('\t'),
                    Some(other) => out.push(other),
                    None => return Err("unterminated escape".to_string()),
                },
                Some(ch) => out.push(ch),
                None => return Err("unterminated string".to_string()),
            }
        }
    }

    fn parse_atom(&mut self) -> Result<Sexp, String> {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_whitespace() || ch == '(' || ch == ')' {
                break;
            }
            out.push(ch);
            self.index += 1;
        }
        if out.is_empty() {
            Err("expected atom".to_string())
        } else {
            Ok(Sexp::Atom(out))
        }
    }
}

fn plist_view(sexp: &Sexp) -> Result<&[Sexp], String> {
    match sexp {
        Sexp::List(items) => {
            if items.is_empty() {
                return Ok(items);
            }
            if let Sexp::Atom(atom) = &items[0] {
                if atom.starts_with(':')
                    && items.len() > 1
                    && matches!(items[1], Sexp::Atom(_))
                    && items[1].atom_starts_with_keyword()
                {
                    return Ok(&items[1..]);
                }
            }
            Ok(items)
        }
        _ => Err("expected plist list".to_string()),
    }
}

trait KeywordAtom {
    fn atom_starts_with_keyword(&self) -> bool;
}

impl KeywordAtom for Sexp {
    fn atom_starts_with_keyword(&self) -> bool {
        matches!(self, Sexp::Atom(atom) if atom.starts_with(':'))
    }
}

fn plist_value<'a>(items: &'a [Sexp], key: &str) -> Option<&'a Sexp> {
    let needle = format!(":{key}");
    let mut index = 0;
    while index + 1 < items.len() {
        if let Sexp::Atom(atom) = &items[index] {
            if atom.eq_ignore_ascii_case(&needle) {
                return items.get(index + 1);
            }
        }
        index += 2;
    }
    None
}

fn plist_list<'a>(items: &'a [Sexp], key: &str) -> Option<&'a [Sexp]> {
    match plist_value(items, key) {
        Some(Sexp::List(list)) => Some(list.as_slice()),
        _ => None,
    }
}

fn plist_f64(items: &[Sexp], key: &str) -> Option<f64> {
    plist_value(items, key).and_then(sexp_to_f64)
}

fn plist_i64(items: &[Sexp], key: &str) -> Option<i64> {
    plist_value(items, key).and_then(sexp_to_i64)
}

fn plist_bool(items: &[Sexp], key: &str) -> Option<bool> {
    plist_value(items, key).and_then(sexp_to_bool)
}

fn plist_string(items: &[Sexp], key: &str) -> Option<String> {
    plist_value(items, key).and_then(sexp_to_string_value)
}

fn sexp_to_f64(sexp: &Sexp) -> Option<f64> {
    match sexp {
        Sexp::Atom(atom) => atom.parse::<f64>().ok(),
        Sexp::String(text) => text.parse::<f64>().ok(),
        Sexp::List(_) => None,
    }
}

fn sexp_to_i64(sexp: &Sexp) -> Option<i64> {
    match sexp {
        Sexp::Atom(atom) => atom
            .parse::<i64>()
            .ok()
            .or_else(|| atom.parse::<f64>().ok().map(|value| value.round() as i64)),
        Sexp::String(text) => text.parse::<i64>().ok(),
        Sexp::List(_) => None,
    }
}

fn sexp_to_bool(sexp: &Sexp) -> Option<bool> {
    match sexp {
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case("t") => Some(true),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case("nil") => Some(false),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case(":true") => Some(true),
        Sexp::Atom(atom) if atom.eq_ignore_ascii_case(":false") => Some(false),
        Sexp::String(text) if text.eq_ignore_ascii_case("true") => Some(true),
        Sexp::String(text) if text.eq_ignore_ascii_case("false") => Some(false),
        _ => None,
    }
}

fn sexp_to_string_value(sexp: &Sexp) -> Option<String> {
    match sexp {
        Sexp::Atom(atom) => Some(atom.clone()),
        Sexp::String(text) => Some(text.clone()),
        Sexp::List(_) => None,
    }
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_init() -> i32 {
    let _ = state();
    ensure_actor_registered();
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_observe(observation_sexp: *const c_char) -> i32 {
    let raw = match cstr_to_string(observation_sexp) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let observation = match parse_observation(&raw) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };

    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };

    let projection = step_kernel(&mut state, &observation);
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    post_projection_signal(&projection);
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_reflect(observation_json: *const c_char) -> i32 {
    harmonia_signalograd_observe(observation_json)
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_feedback(feedback_sexp: *const c_char) -> i32 {
    let raw = match cstr_to_string(feedback_sexp) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let feedback = match parse_feedback(&raw) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };

    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };

    apply_feedback(&mut state, &feedback);
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_checkpoint(path: *const c_char) -> i32 {
    let raw_path = match cstr_to_string(path) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let target = PathBuf::from(raw_path.trim());
    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };

    let body = state_to_sexp(&state);
    state.checkpoint_digest = simple_hash(&body);
    if let Some(parent) = target.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            set_error(err.to_string());
            return -1;
        }
    }
    if let Err(err) = fs::write(&target, body).map_err(|e| e.to_string()) {
        set_error(err);
        return -1;
    }
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_restore(path: *const c_char) -> i32 {
    let raw_path = match cstr_to_string(path) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let target = PathBuf::from(raw_path.trim());
    let restored = match restore_state_from_path(&target) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };

    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };
    *state = restored;
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_status() -> *mut c_char {
    let state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return std::ptr::null_mut();
        }
    };
    clear_error();
    to_c_string(status_sexp(&state))
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_snapshot() -> *mut c_char {
    let state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return std::ptr::null_mut();
        }
    };
    clear_error();
    to_c_string(snapshot_sexp(&state))
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_reset() -> i32 {
    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };
    *state = KernelState::new();
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

#[no_mangle]
pub extern "C" fn harmonia_signalograd_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_sexp_updates_cycle() {
        let obs = parse_observation(
            "(:signalograd-observe :cycle 7 :signal 0.7 :stability 0.8 :novelty 0.2 :security-posture \"nominal\")",
        )
        .expect("parse observation");
        let mut state = KernelState::new();
        let proj = step_kernel(&mut state, &obs);
        assert_eq!(state.cycle, 7);
        assert_eq!(proj.cycle, 7);
    }

    #[test]
    fn proposal_is_bounded() {
        let obs = Observation {
            cycle: 1,
            global_score: 1.0,
            local_score: 1.0,
            signal: 1.0,
            noise: 0.0,
            chaos_risk: 0.0,
            reward: 1.0,
            stability: 1.0,
            novelty: 0.5,
            security_posture: "nominal".to_string(),
            ..Observation::default()
        };
        let mut state = KernelState::new();
        let proj = step_kernel(&mut state, &obs);
        assert!(proj.harmony_signal_bias.abs() <= 0.06);
        assert!(proj.routing_speed_delta.abs() <= 0.07);
        assert!(proj.security_anomaly_delta.abs() <= 0.25);
        assert!(proj.presentation_decor_density_delta.abs() <= 0.25);
    }

    #[test]
    fn checkpoint_round_trip_preserves_state() {
        let obs = Observation {
            cycle: 4,
            signal: 0.8,
            reward: 0.7,
            stability: 0.9,
            novelty: 0.4,
            security_posture: "nominal".to_string(),
            ..Observation::default()
        };
        let mut state = KernelState::new();
        let _ = step_kernel(&mut state, &obs);
        let feedback = Feedback {
            cycle: 4,
            reward: 0.75,
            stability: 0.82,
            novelty: 0.35,
            accepted: true,
            recall_hits: 1,
            user_affinity: 0.8,
            cleanliness: 0.9,
            applied_confidence: 0.6,
        };
        apply_feedback(&mut state, &feedback);
        let dir = std::env::temp_dir().join(format!("signalograd-test-{}", std::process::id()));
        let path = dir.join("state.sexp");
        write_state_to_path(&state, &path).expect("write checkpoint");
        let restored = restore_state_from_path(&path).expect("restore checkpoint");
        assert_eq!(restored.cycle, state.cycle);
        assert_eq!(
            restored.last_feedback.accepted,
            state.last_feedback.accepted
        );
        assert_eq!(restored.memory_slots.len(), MEMORY_SLOTS);
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(dir);
    }
}
