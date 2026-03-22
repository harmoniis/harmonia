use serde::Deserialize;
use std::sync::atomic::AtomicU64;
use std::sync::{Mutex, OnceLock};

use crate::error::seeded_weight;

pub(crate) const LATENT_DIM: usize = 32;
pub(crate) const INPUT_DIM: usize = 25;
pub(crate) const MEMORY_SLOTS: usize = 32;
pub(crate) const HEAD_COUNT: usize = 5;
pub(crate) const PHI: f64 = 1.618_033_988_749_895;
pub(crate) const FEIGENBAUM_DELTA: f64 = 4.669_201_609_102_99;
pub(crate) const FEIGENBAUM_ALPHA: f64 = 2.502_907_875_095_892_6;
pub(crate) const VERSION: &[u8] = b"harmonia-signalograd/0.2.0\0";
pub(crate) const COMPONENT: &str = "signalograd-core";

pub(crate) static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();
pub(crate) static STATE: OnceLock<Mutex<KernelState>> = OnceLock::new();
pub(crate) static ACTOR_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub(crate) struct Observation {
    pub(crate) cycle: i64,
    pub(crate) global_score: f64,
    pub(crate) local_score: f64,
    pub(crate) signal: f64,
    pub(crate) noise: f64,
    pub(crate) chaos_risk: f64,
    pub(crate) rewrite_aggression: f64,
    pub(crate) lorenz_bounded: f64,
    pub(crate) lambdoma_ratio: f64,
    pub(crate) rewrite_ready: bool,
    pub(crate) security_posture: String,
    pub(crate) security_events: f64,
    pub(crate) route_success: f64,
    pub(crate) route_latency: f64,
    pub(crate) cost_pressure: f64,
    pub(crate) memory_pressure: f64,
    pub(crate) graph_density: f64,
    pub(crate) graph_interdisciplinary: f64,
    pub(crate) reward: f64,
    pub(crate) stability: f64,
    pub(crate) novelty: f64,
    pub(crate) actor_load: f64,
    pub(crate) actor_stalls: f64,
    pub(crate) queue_depth: f64,
    pub(crate) error_pressure: f64,
    pub(crate) supervision: f64,
    pub(crate) prior_confidence: f64,
    pub(crate) presentation_cleanliness: f64,
    pub(crate) presentation_verbosity: f64,
    pub(crate) presentation_markdown_density: f64,
    pub(crate) presentation_symbolic_density: f64,
    pub(crate) presentation_self_reference: f64,
    pub(crate) presentation_decor_density: f64,
    pub(crate) presentation_user_affinity: f64,
    #[serde(default)]
    #[allow(dead_code)] // consumed by Lisp via sexp observation, not read in Rust
    pub(crate) route_tier: String,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct Feedback {
    pub(crate) cycle: i64,
    pub(crate) reward: f64,
    pub(crate) stability: f64,
    pub(crate) novelty: f64,
    pub(crate) accepted: bool,
    pub(crate) recall_hits: i64,
    pub(crate) user_affinity: f64,
    pub(crate) cleanliness: f64,
    pub(crate) applied_confidence: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LorenzState {
    pub(crate) x: f64,
    pub(crate) y: f64,
    pub(crate) z: f64,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Projection {
    pub(crate) cycle: i64,
    pub(crate) confidence: f64,
    pub(crate) stability: f64,
    pub(crate) novelty: f64,
    pub(crate) latent_energy: f64,
    pub(crate) recall_strength: f64,
    pub(crate) harmony_signal_bias: f64,
    pub(crate) harmony_noise_bias: f64,
    pub(crate) rewrite_signal_delta: f64,
    pub(crate) rewrite_chaos_delta: f64,
    pub(crate) evolution_aggression_bias: f64,
    pub(crate) routing_price_delta: f64,
    pub(crate) routing_speed_delta: f64,
    pub(crate) routing_success_delta: f64,
    pub(crate) routing_reasoning_delta: f64,
    pub(crate) routing_vitruvian_min_delta: f64,
    pub(crate) memory_recall_limit_delta: i32,
    pub(crate) memory_crystal_threshold_delta: f64,
    pub(crate) security_dissonance_delta: f64,
    pub(crate) security_anomaly_delta: f64,
    pub(crate) presentation_verbosity_delta: f64,
    pub(crate) presentation_markdown_density_delta: f64,
    pub(crate) presentation_symbolic_density_delta: f64,
    pub(crate) presentation_self_reference_delta: f64,
    pub(crate) presentation_decor_density_delta: f64,
}

#[derive(Clone, Debug)]
pub(crate) struct KernelState {
    pub(crate) cycle: i64,
    pub(crate) lorenz: LorenzState,
    pub(crate) latent: [f64; LATENT_DIM],
    pub(crate) input_matrix: Vec<f64>,
    pub(crate) recurrent_matrix: Vec<f64>,
    pub(crate) readout_weights: Vec<Vec<f64>>,
    pub(crate) memory_slots: Vec<Vec<f64>>,
    pub(crate) memory_strengths: Vec<f64>,
    pub(crate) memory_usage: Vec<f64>,
    pub(crate) last_projection: Projection,
    pub(crate) last_feedback: Feedback,
    pub(crate) last_observation: Observation,
    pub(crate) last_recall_strength: f64,
    pub(crate) checkpoint_digest: u64,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(crate) struct LegacyKernelState {
    pub(crate) cycle: i64,
    pub(crate) latent: Vec<f64>,
    pub(crate) last_projection: LegacyProjection,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub(crate) struct LegacyProjection {
    pub(crate) cycle: i64,
    pub(crate) confidence: f64,
    pub(crate) stability: f64,
    pub(crate) novelty: f64,
    pub(crate) latent_energy: f64,
    pub(crate) harmony_signal_bias: f64,
    pub(crate) harmony_noise_bias: f64,
    pub(crate) rewrite_signal_delta: f64,
    pub(crate) rewrite_chaos_delta: f64,
    pub(crate) evolution_aggression_bias: f64,
    pub(crate) routing_price_delta: f64,
    pub(crate) routing_speed_delta: f64,
    pub(crate) routing_success_delta: f64,
    pub(crate) routing_reasoning_delta: f64,
    pub(crate) routing_vitruvian_min_delta: f64,
    pub(crate) memory_recall_limit_delta: i32,
    pub(crate) memory_crystal_threshold_delta: f64,
    pub(crate) security_dissonance_delta: f64,
    pub(crate) security_anomaly_delta: f64,
    pub(crate) presentation_verbosity_delta: f64,
    pub(crate) presentation_markdown_density_delta: f64,
    pub(crate) presentation_symbolic_density_delta: f64,
    pub(crate) presentation_self_reference_delta: f64,
    pub(crate) presentation_decor_density_delta: f64,
}

impl KernelState {
    pub(crate) fn new() -> Self {
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

    pub(crate) fn used_memory_slots(&self) -> usize {
        self.memory_strengths
            .iter()
            .filter(|strength| **strength > 0.001)
            .count()
    }
}
