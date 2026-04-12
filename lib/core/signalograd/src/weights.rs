//! Signalograd weight configuration -- the Kolmogorov-compressed weight space.
//!
//! Every coefficient in the kernel is defined here as either:
//! - A safety bound (const, never learned)
//! - A mathematical constant (const, truly constant)
//! - An initial condition for a learned weight (const INIT_*, starting point)
//!
//! Dynamic weights start from initial conditions and evolve through Hebbian
//! learning based on feedback from actual problem-solving. They are stored in
//! `KernelState::dynamic_weights` and persisted in the signalograd.sexp checkpoint.
//!
//! The common structure (convex mixing): most projection deltas follow
//! delta = clamp((alpha * signal_a + beta * signal_b) * scale, -scale, scale)
//! where alpha + beta ~ 1.0. This is the irreducible pattern.

// ---- Mathematical Constants (truly constant, not learned) -----------------
pub const LORENZ_SIGMA_BASE: f64 = 10.0;
pub const LORENZ_RHO_BASE: f64 = 28.0;
pub const LORENZ_BETA_BASE: f64 = 8.0 / 3.0;

// ---- Safety Bounds (const, never learned) ---------------------------------
pub const LORENZ_X_BOUND: f64 = 40.0;
pub const LORENZ_Y_BOUND: f64 = 50.0;
pub const LORENZ_Z_MIN: f64 = 0.0;
pub const LORENZ_Z_MAX: f64 = 60.0;
pub const WEIGHT_BOUND: f64 = 2.5;

// ---- Dynamic Weight Layout ------------------------------------------------
// Total number of dynamically learned weights in signalograd.
pub const DYNAMIC_WEIGHT_COUNT: usize = 119;

// Weight group layout indices into KernelState::dynamic_weights
pub const DW_LORENZ_START: usize = 0;
pub const DW_LORENZ_COUNT: usize = 14;
pub const DW_LORENZ_AUX_START: usize = 14;
pub const DW_LORENZ_AUX_COUNT: usize = 4;
pub const DW_HOPFIELD_START: usize = 18;
pub const DW_HOPFIELD_COUNT: usize = 4;
pub const DW_LEARNING_START: usize = 22;
pub const DW_LEARNING_COUNT: usize = 4;
pub const DW_LATENT_START: usize = 26;
pub const DW_LATENT_COUNT: usize = 3;
pub const DW_CONFIDENCE_START: usize = 29;
pub const DW_CONFIDENCE_COUNT: usize = 6;
pub const DW_HEAD_START: usize = 35;
pub const DW_HEAD_COUNT: usize = 27;
pub const DW_PROJ_SCALE_START: usize = 62;
pub const DW_PROJ_SCALE_COUNT: usize = 16;
pub const DW_PROJ_ALPHA_START: usize = 78;
pub const DW_PROJ_ALPHA_COUNT: usize = 11;
pub const DW_ROUTING_REASONING_START: usize = 89;
pub const DW_ROUTING_REASONING_COUNT: usize = 4;
pub const DW_ROUTING_VITRUVIAN_START: usize = 93;
pub const DW_ROUTING_VITRUVIAN_COUNT: usize = 2;
pub const DW_MEMORY_RECALL_LIMIT_START: usize = 95;
pub const DW_MEMORY_RECALL_LIMIT_COUNT: usize = 3;
pub const DW_PRES_MIX_START: usize = 98;
pub const DW_PRES_MIX_COUNT: usize = 18;
pub const DW_INIT_SCALE_START: usize = 116;
pub const DW_INIT_SCALE_COUNT: usize = 3;

// ---- Initial Conditions for Learned Weights (const, starting point) -------
// These are loaded into KernelState::dynamic_weights at construction time.
// The system learns better values through continuous Hebbian inference.

// -- Lorenz Modulation (14 weights, indices 0..14) --
/// Initial condition for Lorenz sigma-signal coupling. Learned by signalograd.
pub const INIT_LORENZ_SIGMA_SIGNAL_SCALE: f64 = 2.0;
/// Initial condition for Lorenz rho-score coupling. Learned by signalograd.
pub const INIT_LORENZ_RHO_SCORE_SCALE: f64 = 5.0;
/// Initial condition for Lorenz rho-route coupling. Learned by signalograd.
pub const INIT_LORENZ_RHO_ROUTE_SCALE: f64 = 2.0;
/// Initial condition for Lorenz beta-memory coupling. Learned by signalograd.
pub const INIT_LORENZ_BETA_MEMORY_SCALE: f64 = 0.08;
/// Initial condition for Lorenz dt base. Learned by signalograd.
pub const INIT_LORENZ_DT_BASE: f64 = 0.008;
/// Initial condition for Lorenz dt-stability coupling. Learned by signalograd.
pub const INIT_LORENZ_DT_STABILITY_SCALE: f64 = 0.004;
/// Initial condition for Lorenz dt-novelty coupling. Learned by signalograd.
pub const INIT_LORENZ_DT_NOVELTY_SCALE: f64 = 0.002;
/// Initial condition for Lorenz dt minimum. Learned by signalograd.
pub const INIT_LORENZ_DT_MIN: f64 = 0.004;
/// Initial condition for Lorenz dt maximum. Learned by signalograd.
pub const INIT_LORENZ_DT_MAX: f64 = 0.02;
/// Initial condition for Lorenz dx-signal coupling. Learned by signalograd.
pub const INIT_LORENZ_DX_SIGNAL_COUPLING: f64 = 0.35;
/// Initial condition for Lorenz dx-actor coupling. Learned by signalograd.
pub const INIT_LORENZ_DX_ACTOR_COUPLING: f64 = 0.08;
/// Initial condition for Lorenz dy-route coupling. Learned by signalograd.
pub const INIT_LORENZ_DY_ROUTE_COUPLING: f64 = 0.22;
/// Initial condition for Lorenz dy-cost coupling. Learned by signalograd.
pub const INIT_LORENZ_DY_COST_COUPLING: f64 = 0.12;
/// Initial condition for Lorenz dz-graph coupling. Learned by signalograd.
pub const INIT_LORENZ_DZ_GRAPH_COUPLING: f64 = 0.18;

// -- Lorenz Auxiliary (4 weights, indices 14..18) --
/// Initial condition for Lorenz energy normalization. Learned by signalograd.
pub const INIT_LORENZ_ENERGY_NORMALIZATION: f64 = 40.0;
/// Initial condition for Lorenz basis y-phase. Learned by signalograd.
pub const INIT_LORENZ_BASIS_Y_PHASE: f64 = 0.7;
/// Initial condition for Lorenz basis z-phase. Learned by signalograd.
pub const INIT_LORENZ_BASIS_Z_PHASE: f64 = 0.3;
/// Initial condition for Lorenz basis scale. Learned by signalograd.
pub const INIT_LORENZ_BASIS_SCALE: f64 = 0.018;

// -- Hopfield Memory (4 weights, indices 18..22) --
/// Initial condition for Hopfield strength threshold. Learned by signalograd.
pub const INIT_HOPFIELD_STRENGTH_THRESHOLD: f64 = 0.001;
/// Initial condition for Hopfield similarity base. Learned by signalograd.
pub const INIT_HOPFIELD_SIMILARITY_BASE: f64 = 0.65;
/// Initial condition for Hopfield strength scale. Learned by signalograd.
pub const INIT_HOPFIELD_STRENGTH_SCALE: f64 = 0.35;
/// Initial condition for Hopfield recall active threshold. Learned by signalograd.
pub const INIT_HOPFIELD_RECALL_ACTIVE_THRESHOLD: f64 = 0.12;

// -- Learning (4 weights, indices 22..26) --
/// Initial condition for Oja regularization. Learned by signalograd.
pub const INIT_OJA_REGULARIZATION: f64 = 0.03;
/// Initial condition for weight decay. Learned by signalograd.
pub const INIT_WEIGHT_DECAY: f64 = 0.998;
/// Initial condition for learning rate. Learned by signalograd.
pub const INIT_LEARNING_RATE: f64 = 0.045;
/// Initial condition for memory usage decay. Learned by signalograd.
pub const INIT_MEMORY_USAGE_DECAY: f64 = 0.995;

// -- Latent (3 weights, indices 26..29) --
/// Initial condition for latent mean subtraction. Learned by signalograd.
pub const INIT_LATENT_MEAN_SUBTRACTION: f64 = 0.12;
/// Initial condition for latent recurrence coefficient. Learned by signalograd.
pub const INIT_LATENT_RECURRENCE_COEFF: f64 = 0.42;
/// Initial condition for latent recall coupling. Learned by signalograd.
pub const INIT_LATENT_RECALL_COUPLING: f64 = 0.14;

// -- Confidence (6 weights, indices 29..35) --
/// Initial condition for confidence stability weight. Learned by signalograd.
pub const INIT_CONFIDENCE_STABILITY_W: f64 = 0.35;
/// Initial condition for confidence anti-chaos weight. Learned by signalograd.
pub const INIT_CONFIDENCE_ANTICHAOS_W: f64 = 0.20;
/// Initial condition for confidence anti-noise weight. Learned by signalograd.
pub const INIT_CONFIDENCE_ANTINOISE_W: f64 = 0.15;
/// Initial condition for confidence recall weight. Learned by signalograd.
pub const INIT_CONFIDENCE_RECALL_W: f64 = 0.15;
/// Initial condition for confidence lorenz weight. Learned by signalograd.
pub const INIT_CONFIDENCE_LORENZ_W: f64 = 0.10;
/// Initial condition for confidence cleanliness weight. Learned by signalograd.
pub const INIT_CONFIDENCE_CLEANLINESS_W: f64 = 0.05;

// -- Head Target Weights (27 weights, indices 35..62) --
// Harmony head (8)
pub const INIT_HARMONY_REWARD_W: f64 = 0.25;
pub const INIT_HARMONY_RECALL_W: f64 = 0.15;
pub const INIT_HARMONY_LORENZ_W: f64 = 0.10;
pub const INIT_HARMONY_CHAOS_W: f64 = -0.20;
pub const INIT_HARMONY_CLEANLINESS_W: f64 = 0.12;
pub const INIT_HARMONY_AFFINITY_W: f64 = 0.08;
pub const INIT_HARMONY_SYMBOLIC_W: f64 = -0.10;
pub const INIT_HARMONY_DECOR_W: f64 = -0.10;
// Routing head (4)
pub const INIT_ROUTING_COST_W: f64 = -0.55;
pub const INIT_ROUTING_LATENCY_W: f64 = -0.35;
pub const INIT_ROUTING_CONFIDENCE_W: f64 = 0.20;
pub const INIT_ROUTING_RECALL_W: f64 = 0.10;
// Memory head (8)
pub const INIT_MEMORY_PRESSURE_W: f64 = 0.40;
pub const INIT_MEMORY_STABILITY_W: f64 = 0.18;
pub const INIT_MEMORY_RECALL_W: f64 = 0.10;
pub const INIT_MEMORY_FIELD_RECALL_W: f64 = 0.10;
pub const INIT_MEMORY_BASIN_W: f64 = 0.08;
pub const INIT_MEMORY_EIGENMODE_W: f64 = 0.07;
pub const INIT_MEMORY_AFFINITY_W: f64 = 0.07;
pub const INIT_MEMORY_VERBOSITY_W: f64 = -0.08;
// Evolution head (4)
pub const INIT_EVOLUTION_REWRITE_READY_BONUS: f64 = 0.25;
pub const INIT_EVOLUTION_REWRITE_READY_PENALTY: f64 = -0.10;
pub const INIT_EVOLUTION_STABILITY_W: f64 = 0.15;
pub const INIT_EVOLUTION_CLEANLINESS_W: f64 = 0.10;
// Security head (3)
pub const INIT_SECURITY_NOISE_W: f64 = -0.20;
pub const INIT_SECURITY_ERROR_W: f64 = -0.15;
pub const INIT_SECURITY_DECOR_W: f64 = -0.08;

// -- Projection Scale Factors (16 weights, indices 62..78) --
pub const INIT_PROJ_HARMONY_SIGNAL_SCALE: f64 = 0.06;
pub const INIT_PROJ_HARMONY_NOISE_SCALE: f64 = 0.04;
pub const INIT_PROJ_REWRITE_SIGNAL_SCALE: f64 = 0.05;
pub const INIT_PROJ_REWRITE_CHAOS_SCALE: f64 = 0.04;
pub const INIT_PROJ_EVOLUTION_AGGRESSION_SCALE: f64 = 0.08;
pub const INIT_PROJ_ROUTING_PRICE_SCALE: f64 = 0.07;
pub const INIT_PROJ_ROUTING_SPEED_SCALE: f64 = 0.07;
pub const INIT_PROJ_ROUTING_SUCCESS_SCALE: f64 = 0.05;
pub const INIT_PROJ_MEMORY_CRYSTAL_SCALE: f64 = 0.05;
pub const INIT_PROJ_SECURITY_DISSONANCE_SCALE: f64 = 0.03;
pub const INIT_PROJ_SECURITY_ANOMALY_SCALE: f64 = 0.25;
pub const INIT_PROJ_PRESENTATION_VERBOSITY_SCALE: f64 = 0.22;
pub const INIT_PROJ_PRESENTATION_MARKDOWN_SCALE: f64 = 0.18;
pub const INIT_PROJ_PRESENTATION_SYMBOLIC_SCALE: f64 = 0.22;
pub const INIT_PROJ_PRESENTATION_SELFREF_SCALE: f64 = 0.22;
pub const INIT_PROJ_PRESENTATION_DECOR_SCALE: f64 = 0.25;

// -- Projection Mixing Alphas (11 weights, indices 78..89) --
pub const INIT_PROJ_HARMONY_SIGNAL_ALPHA: f64 = 0.80;
pub const INIT_PROJ_HARMONY_NOISE_ALPHA: f64 = 0.70;
pub const INIT_PROJ_REWRITE_SIGNAL_ALPHA: f64 = 0.75;
pub const INIT_PROJ_REWRITE_CHAOS_ALPHA: f64 = 0.70;
pub const INIT_PROJ_EVOLUTION_AGGRESSION_ALPHA: f64 = 0.75;
pub const INIT_PROJ_ROUTING_PRICE_ALPHA: f64 = 0.70;
pub const INIT_PROJ_ROUTING_SPEED_ALPHA: f64 = 0.75;
pub const INIT_PROJ_ROUTING_SUCCESS_ALPHA: f64 = 0.65;
pub const INIT_PROJ_MEMORY_CRYSTAL_ALPHA: f64 = 0.75;
pub const INIT_PROJ_SECURITY_DISSONANCE_ALPHA: f64 = 0.80;
pub const INIT_PROJ_SECURITY_ANOMALY_ALPHA: f64 = 0.70;

// -- Routing Reasoning Projection (4 weights, indices 89..93) --
pub const INIT_PROJ_ROUTING_REASONING_ROUTING_W: f64 = 0.6;
pub const INIT_PROJ_ROUTING_REASONING_HARMONY_W: f64 = 0.25;
pub const INIT_PROJ_ROUTING_REASONING_RECALL_W: f64 = 0.15;
pub const INIT_PROJ_ROUTING_REASONING_SCALE: f64 = 0.06;

// -- Routing Vitruvian Min Projection (2 weights, indices 93..95) --
pub const INIT_PROJ_ROUTING_VITRUVIAN_ALPHA: f64 = 0.7;
pub const INIT_PROJ_ROUTING_VITRUVIAN_SCALE: f64 = 0.04;

// -- Memory Recall Limit Projection (3 weights, indices 95..98) --
pub const INIT_PROJ_MEMORY_RECALL_HEAD_SCALE: f64 = 2.0;
pub const INIT_PROJ_MEMORY_RECALL_STRENGTH_SCALE: f64 = 1.5;
pub const INIT_PROJ_MEMORY_RECALL_LIMIT_BOUND: f64 = 2.0;

// -- Presentation Projection Mixing Weights (18 weights, indices 98..116) --
// Verbosity (4)
pub const INIT_PROJ_PRES_VERBOSITY_MEMORY_W: f64 = 0.50;
pub const INIT_PROJ_PRES_VERBOSITY_CURRENT_W: f64 = -0.45;
pub const INIT_PROJ_PRES_VERBOSITY_AFFINITY_W: f64 = 0.20;
pub const INIT_PROJ_PRES_VERBOSITY_CLEAN_W: f64 = 0.15;
// Markdown (3)
pub const INIT_PROJ_PRES_MARKDOWN_MEMORY_W: f64 = 0.40;
pub const INIT_PROJ_PRES_MARKDOWN_CURRENT_W: f64 = -0.50;
pub const INIT_PROJ_PRES_MARKDOWN_AFFINITY_W: f64 = 0.20;
// Symbolic (4)
pub const INIT_PROJ_PRES_SYMBOLIC_CURRENT_W: f64 = -0.65;
pub const INIT_PROJ_PRES_SYMBOLIC_HARMONY_W: f64 = 0.20;
pub const INIT_PROJ_PRES_SYMBOLIC_RECALL_W: f64 = 0.15;
pub const INIT_PROJ_PRES_SYMBOLIC_CLEAN_W: f64 = 0.20;
// Self-reference (4)
pub const INIT_PROJ_PRES_SELFREF_CURRENT_W: f64 = -0.70;
pub const INIT_PROJ_PRES_SELFREF_HARMONY_W: f64 = 0.15;
pub const INIT_PROJ_PRES_SELFREF_AFFINITY_W: f64 = 0.15;
pub const INIT_PROJ_PRES_SELFREF_CLEAN_W: f64 = 0.10;
// Decor (3)
pub const INIT_PROJ_PRES_DECOR_CURRENT_W: f64 = -0.80;
pub const INIT_PROJ_PRES_DECOR_UNCLEAN_W: f64 = -0.35;
pub const INIT_PROJ_PRES_DECOR_HARMONY_W: f64 = 0.15;

// -- Network Initialization Scales (3 weights, indices 116..119) --
pub const INIT_INPUT_SCALE: f64 = 0.19;
pub const INIT_RECURRENT_SCALE: f64 = 0.11;
pub const INIT_READOUT_SCALE: f64 = 0.08;

// ---- Dynamic Learning Rate ------------------------------------------------
/// Learning rate for Hebbian update of dynamic weights. Low for stability.
pub const INIT_DYNAMIC_LEARNING_RATE: f64 = 0.01;

/// Build the initial dynamic_weights vector from all INIT_* constants.
/// Layout exactly matches the DW_* index constants above.
pub fn initial_dynamic_weights() -> Vec<f64> {
    let mut dw = vec![0.0; DYNAMIC_WEIGHT_COUNT];

    // Lorenz modulation (14)
    dw[0] = INIT_LORENZ_SIGMA_SIGNAL_SCALE;
    dw[1] = INIT_LORENZ_RHO_SCORE_SCALE;
    dw[2] = INIT_LORENZ_RHO_ROUTE_SCALE;
    dw[3] = INIT_LORENZ_BETA_MEMORY_SCALE;
    dw[4] = INIT_LORENZ_DT_BASE;
    dw[5] = INIT_LORENZ_DT_STABILITY_SCALE;
    dw[6] = INIT_LORENZ_DT_NOVELTY_SCALE;
    dw[7] = INIT_LORENZ_DT_MIN;
    dw[8] = INIT_LORENZ_DT_MAX;
    dw[9] = INIT_LORENZ_DX_SIGNAL_COUPLING;
    dw[10] = INIT_LORENZ_DX_ACTOR_COUPLING;
    dw[11] = INIT_LORENZ_DY_ROUTE_COUPLING;
    dw[12] = INIT_LORENZ_DY_COST_COUPLING;
    dw[13] = INIT_LORENZ_DZ_GRAPH_COUPLING;

    // Lorenz auxiliary (4)
    dw[14] = INIT_LORENZ_ENERGY_NORMALIZATION;
    dw[15] = INIT_LORENZ_BASIS_Y_PHASE;
    dw[16] = INIT_LORENZ_BASIS_Z_PHASE;
    dw[17] = INIT_LORENZ_BASIS_SCALE;

    // Hopfield (4)
    dw[18] = INIT_HOPFIELD_STRENGTH_THRESHOLD;
    dw[19] = INIT_HOPFIELD_SIMILARITY_BASE;
    dw[20] = INIT_HOPFIELD_STRENGTH_SCALE;
    dw[21] = INIT_HOPFIELD_RECALL_ACTIVE_THRESHOLD;

    // Learning (4)
    dw[22] = INIT_OJA_REGULARIZATION;
    dw[23] = INIT_WEIGHT_DECAY;
    dw[24] = INIT_LEARNING_RATE;
    dw[25] = INIT_MEMORY_USAGE_DECAY;

    // Latent (3)
    dw[26] = INIT_LATENT_MEAN_SUBTRACTION;
    dw[27] = INIT_LATENT_RECURRENCE_COEFF;
    dw[28] = INIT_LATENT_RECALL_COUPLING;

    // Confidence (6)
    dw[29] = INIT_CONFIDENCE_STABILITY_W;
    dw[30] = INIT_CONFIDENCE_ANTICHAOS_W;
    dw[31] = INIT_CONFIDENCE_ANTINOISE_W;
    dw[32] = INIT_CONFIDENCE_RECALL_W;
    dw[33] = INIT_CONFIDENCE_LORENZ_W;
    dw[34] = INIT_CONFIDENCE_CLEANLINESS_W;

    // Head targets (27)
    dw[35] = INIT_HARMONY_REWARD_W;
    dw[36] = INIT_HARMONY_RECALL_W;
    dw[37] = INIT_HARMONY_LORENZ_W;
    dw[38] = INIT_HARMONY_CHAOS_W;
    dw[39] = INIT_HARMONY_CLEANLINESS_W;
    dw[40] = INIT_HARMONY_AFFINITY_W;
    dw[41] = INIT_HARMONY_SYMBOLIC_W;
    dw[42] = INIT_HARMONY_DECOR_W;
    dw[43] = INIT_ROUTING_COST_W;
    dw[44] = INIT_ROUTING_LATENCY_W;
    dw[45] = INIT_ROUTING_CONFIDENCE_W;
    dw[46] = INIT_ROUTING_RECALL_W;
    dw[47] = INIT_MEMORY_PRESSURE_W;
    dw[48] = INIT_MEMORY_STABILITY_W;
    dw[49] = INIT_MEMORY_RECALL_W;
    dw[50] = INIT_MEMORY_FIELD_RECALL_W;
    dw[51] = INIT_MEMORY_BASIN_W;
    dw[52] = INIT_MEMORY_EIGENMODE_W;
    dw[53] = INIT_MEMORY_AFFINITY_W;
    dw[54] = INIT_MEMORY_VERBOSITY_W;
    dw[55] = INIT_EVOLUTION_REWRITE_READY_BONUS;
    dw[56] = INIT_EVOLUTION_REWRITE_READY_PENALTY;
    dw[57] = INIT_EVOLUTION_STABILITY_W;
    dw[58] = INIT_EVOLUTION_CLEANLINESS_W;
    dw[59] = INIT_SECURITY_NOISE_W;
    dw[60] = INIT_SECURITY_ERROR_W;
    dw[61] = INIT_SECURITY_DECOR_W;

    // Projection scales (16)
    dw[62] = INIT_PROJ_HARMONY_SIGNAL_SCALE;
    dw[63] = INIT_PROJ_HARMONY_NOISE_SCALE;
    dw[64] = INIT_PROJ_REWRITE_SIGNAL_SCALE;
    dw[65] = INIT_PROJ_REWRITE_CHAOS_SCALE;
    dw[66] = INIT_PROJ_EVOLUTION_AGGRESSION_SCALE;
    dw[67] = INIT_PROJ_ROUTING_PRICE_SCALE;
    dw[68] = INIT_PROJ_ROUTING_SPEED_SCALE;
    dw[69] = INIT_PROJ_ROUTING_SUCCESS_SCALE;
    dw[70] = INIT_PROJ_MEMORY_CRYSTAL_SCALE;
    dw[71] = INIT_PROJ_SECURITY_DISSONANCE_SCALE;
    dw[72] = INIT_PROJ_SECURITY_ANOMALY_SCALE;
    dw[73] = INIT_PROJ_PRESENTATION_VERBOSITY_SCALE;
    dw[74] = INIT_PROJ_PRESENTATION_MARKDOWN_SCALE;
    dw[75] = INIT_PROJ_PRESENTATION_SYMBOLIC_SCALE;
    dw[76] = INIT_PROJ_PRESENTATION_SELFREF_SCALE;
    dw[77] = INIT_PROJ_PRESENTATION_DECOR_SCALE;

    // Projection alphas (11)
    dw[78] = INIT_PROJ_HARMONY_SIGNAL_ALPHA;
    dw[79] = INIT_PROJ_HARMONY_NOISE_ALPHA;
    dw[80] = INIT_PROJ_REWRITE_SIGNAL_ALPHA;
    dw[81] = INIT_PROJ_REWRITE_CHAOS_ALPHA;
    dw[82] = INIT_PROJ_EVOLUTION_AGGRESSION_ALPHA;
    dw[83] = INIT_PROJ_ROUTING_PRICE_ALPHA;
    dw[84] = INIT_PROJ_ROUTING_SPEED_ALPHA;
    dw[85] = INIT_PROJ_ROUTING_SUCCESS_ALPHA;
    dw[86] = INIT_PROJ_MEMORY_CRYSTAL_ALPHA;
    dw[87] = INIT_PROJ_SECURITY_DISSONANCE_ALPHA;
    dw[88] = INIT_PROJ_SECURITY_ANOMALY_ALPHA;

    // Routing reasoning (4)
    dw[89] = INIT_PROJ_ROUTING_REASONING_ROUTING_W;
    dw[90] = INIT_PROJ_ROUTING_REASONING_HARMONY_W;
    dw[91] = INIT_PROJ_ROUTING_REASONING_RECALL_W;
    dw[92] = INIT_PROJ_ROUTING_REASONING_SCALE;

    // Routing vitruvian (2)
    dw[93] = INIT_PROJ_ROUTING_VITRUVIAN_ALPHA;
    dw[94] = INIT_PROJ_ROUTING_VITRUVIAN_SCALE;

    // Memory recall limit (3)
    dw[95] = INIT_PROJ_MEMORY_RECALL_HEAD_SCALE;
    dw[96] = INIT_PROJ_MEMORY_RECALL_STRENGTH_SCALE;
    dw[97] = INIT_PROJ_MEMORY_RECALL_LIMIT_BOUND;

    // Presentation mixing (18)
    dw[98] = INIT_PROJ_PRES_VERBOSITY_MEMORY_W;
    dw[99] = INIT_PROJ_PRES_VERBOSITY_CURRENT_W;
    dw[100] = INIT_PROJ_PRES_VERBOSITY_AFFINITY_W;
    dw[101] = INIT_PROJ_PRES_VERBOSITY_CLEAN_W;
    dw[102] = INIT_PROJ_PRES_MARKDOWN_MEMORY_W;
    dw[103] = INIT_PROJ_PRES_MARKDOWN_CURRENT_W;
    dw[104] = INIT_PROJ_PRES_MARKDOWN_AFFINITY_W;
    dw[105] = INIT_PROJ_PRES_SYMBOLIC_CURRENT_W;
    dw[106] = INIT_PROJ_PRES_SYMBOLIC_HARMONY_W;
    dw[107] = INIT_PROJ_PRES_SYMBOLIC_RECALL_W;
    dw[108] = INIT_PROJ_PRES_SYMBOLIC_CLEAN_W;
    dw[109] = INIT_PROJ_PRES_SELFREF_CURRENT_W;
    dw[110] = INIT_PROJ_PRES_SELFREF_HARMONY_W;
    dw[111] = INIT_PROJ_PRES_SELFREF_AFFINITY_W;
    dw[112] = INIT_PROJ_PRES_SELFREF_CLEAN_W;
    dw[113] = INIT_PROJ_PRES_DECOR_CURRENT_W;
    dw[114] = INIT_PROJ_PRES_DECOR_UNCLEAN_W;
    dw[115] = INIT_PROJ_PRES_DECOR_HARMONY_W;

    // Network initialization scales (3)
    dw[116] = INIT_INPUT_SCALE;
    dw[117] = INIT_RECURRENT_SCALE;
    dw[118] = INIT_READOUT_SCALE;

    dw
}
