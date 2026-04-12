use serde::Deserialize;

use crate::error::seeded_weight;
use crate::weights;

pub const LATENT_DIM: usize = 32;
pub const INPUT_DIM: usize = 31;
pub const MEMORY_SLOTS: usize = 32;
pub const HEAD_COUNT: usize = 5;
pub const PHI: f64 = 1.618_033_988_749_895;
pub const FEIGENBAUM_DELTA: f64 = 4.669_201_609_102_99;
pub const FEIGENBAUM_ALPHA: f64 = 2.502_907_875_095_892_6;
pub const VERSION: &[u8] = b"harmonia-signalograd/0.2.0\0";
pub const COMPONENT: &str = "signalograd-core";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Observation {
    pub cycle: i64,
    pub global_score: f64,
    pub local_score: f64,
    pub signal: f64,
    pub noise: f64,
    pub chaos_risk: f64,
    pub rewrite_aggression: f64,
    pub lorenz_bounded: f64,
    pub lambdoma_ratio: f64,
    pub rewrite_ready: bool,
    pub security_posture: String,
    pub security_events: f64,
    pub route_success: f64,
    pub route_latency: f64,
    pub cost_pressure: f64,
    pub memory_pressure: f64,
    pub graph_density: f64,
    pub graph_interdisciplinary: f64,
    pub reward: f64,
    pub stability: f64,
    pub novelty: f64,
    pub actor_load: f64,
    pub actor_stalls: f64,
    pub queue_depth: f64,
    pub error_pressure: f64,
    pub supervision: f64,
    pub prior_confidence: f64,
    // Memory-field feedback metrics.
    pub field_recall_strength: f64,
    pub field_basin_stability: f64,
    pub field_eigenmode_coherence: f64,
    pub presentation_cleanliness: f64,
    pub presentation_verbosity: f64,
    pub presentation_markdown_density: f64,
    pub presentation_symbolic_density: f64,
    pub presentation_self_reference: f64,
    pub presentation_decor_density: f64,
    pub presentation_user_affinity: f64,
    #[serde(default)]
    pub route_tier: String,
    // Datamining feedback metrics (terraphon + mempalace).
    pub datamine_success_rate: f64,
    pub datamine_avg_latency: f64,
    pub palace_graph_density: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Feedback {
    pub cycle: i64,
    pub reward: f64,
    pub stability: f64,
    pub novelty: f64,
    pub accepted: bool,
    pub recall_hits: i64,
    pub user_affinity: f64,
    pub cleanliness: f64,
    pub applied_confidence: f64,
}

#[derive(Debug, Clone, Default)]
pub struct LorenzState {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Projection {
    pub cycle: i64,
    pub confidence: f64,
    pub stability: f64,
    pub novelty: f64,
    pub latent_energy: f64,
    pub recall_strength: f64,
    pub harmony_signal_bias: f64,
    pub harmony_noise_bias: f64,
    pub rewrite_signal_delta: f64,
    pub rewrite_chaos_delta: f64,
    pub evolution_aggression_bias: f64,
    pub routing_price_delta: f64,
    pub routing_speed_delta: f64,
    pub routing_success_delta: f64,
    pub routing_reasoning_delta: f64,
    pub routing_vitruvian_min_delta: f64,
    pub memory_recall_limit_delta: i32,
    pub memory_crystal_threshold_delta: f64,
    pub security_dissonance_delta: f64,
    pub security_anomaly_delta: f64,
    pub presentation_verbosity_delta: f64,
    pub presentation_markdown_density_delta: f64,
    pub presentation_symbolic_density_delta: f64,
    pub presentation_self_reference_delta: f64,
    pub presentation_decor_density_delta: f64,
}

#[derive(Clone, Debug)]
pub struct KernelState {
    pub cycle: i64,
    pub lorenz: LorenzState,
    pub latent: [f64; LATENT_DIM],
    pub input_matrix: Vec<f64>,
    pub recurrent_matrix: Vec<f64>,
    pub readout_weights: Vec<Vec<f64>>,
    pub memory_slots: Vec<Vec<f64>>,
    pub memory_strengths: Vec<f64>,
    pub memory_usage: Vec<f64>,
    /// Dynamically learned weights for Lorenz modulation, head targets, and projections.
    /// These start from initial conditions (weights.rs) and evolve through Hebbian
    /// learning based on feedback from actual problem-solving.
    ///
    /// Layout: [lorenz(14), lorenz_aux(4), hopfield(4), learning(4), latent(3),
    ///          confidence(6), head_targets(27), proj_scales(16), proj_alphas(11),
    ///          routing_reasoning(4), routing_vitruvian(2), memory_recall_limit(3),
    ///          presentation_mix(18), init_scales(3)]
    /// Total: 119 learnable parameters
    pub dynamic_weights: Vec<f64>,
    pub last_projection: Projection,
    pub last_feedback: Feedback,
    pub last_observation: Observation,
    pub last_recall_strength: f64,
    pub checkpoint_digest: u64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct LegacyKernelState {
    pub cycle: i64,
    pub latent: Vec<f64>,
    pub last_projection: LegacyProjection,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct LegacyProjection {
    pub cycle: i64,
    pub confidence: f64,
    pub stability: f64,
    pub novelty: f64,
    pub latent_energy: f64,
    pub harmony_signal_bias: f64,
    pub harmony_noise_bias: f64,
    pub rewrite_signal_delta: f64,
    pub rewrite_chaos_delta: f64,
    pub evolution_aggression_bias: f64,
    pub routing_price_delta: f64,
    pub routing_speed_delta: f64,
    pub routing_success_delta: f64,
    pub routing_reasoning_delta: f64,
    pub routing_vitruvian_min_delta: f64,
    pub memory_recall_limit_delta: i32,
    pub memory_crystal_threshold_delta: f64,
    pub security_dissonance_delta: f64,
    pub security_anomaly_delta: f64,
    pub presentation_verbosity_delta: f64,
    pub presentation_markdown_density_delta: f64,
    pub presentation_symbolic_density_delta: f64,
    pub presentation_self_reference_delta: f64,
    pub presentation_decor_density_delta: f64,
}

impl KernelState {
    pub fn new() -> Self {
        let dynamic_weights = weights::initial_dynamic_weights();
        let input_scale = dynamic_weights[weights::DW_INIT_SCALE_START];
        let recurrent_scale = dynamic_weights[weights::DW_INIT_SCALE_START + 1];
        let readout_scale = dynamic_weights[weights::DW_INIT_SCALE_START + 2];

        let mut input_matrix = vec![0.0; LATENT_DIM * INPUT_DIM];
        let mut recurrent_matrix = vec![0.0; LATENT_DIM * LATENT_DIM];
        let mut readout_weights = vec![vec![0.0; LATENT_DIM]; HEAD_COUNT];

        for row in 0..LATENT_DIM {
            for col in 0..INPUT_DIM {
                input_matrix[row * INPUT_DIM + col] =
                    seeded_weight(row, col, input_scale);
            }
            for col in 0..LATENT_DIM {
                recurrent_matrix[row * LATENT_DIM + col] = if (row + col * 2) % 7 == 0 {
                    seeded_weight(row + 13, col + 17, recurrent_scale)
                } else {
                    0.0
                };
            }
            for head in 0..HEAD_COUNT {
                readout_weights[head][row] =
                    seeded_weight(head + 31, row + 37, readout_scale);
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
            dynamic_weights,
            last_projection: Projection::default(),
            last_feedback: Feedback::default(),
            last_observation: Observation::default(),
            last_recall_strength: 0.0,
            checkpoint_digest: 0,
        }
    }

    pub fn used_memory_slots(&self) -> usize {
        let threshold = self.dynamic_weights[weights::DW_HOPFIELD_START]; // hopfield_strength_threshold
        self.memory_strengths
            .iter()
            .filter(|strength| **strength > threshold)
            .count()
    }
}
