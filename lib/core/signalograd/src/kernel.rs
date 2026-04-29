use crate::error::clamp;
use crate::model::{
    KernelState, LorenzState, Observation, Projection, HEAD_COUNT, INPUT_DIM, LATENT_DIM,
    MEMORY_SLOTS, PHI,
};
use crate::observation::{observation_vector, posture_scalar};
use crate::weights;
use crate::weights::*;

pub fn dot(weights: &[f64], latent: &[f64; LATENT_DIM]) -> f64 {
    weights
        .iter()
        .zip(latent.iter())
        .map(|(w, l)| *w * *l)
        .sum::<f64>()
        / LATENT_DIM as f64
}

pub fn normalize_latent(latent: &mut [f64; LATENT_DIM], dw: &[f64]) {
    let mean = latent.iter().sum::<f64>() / LATENT_DIM as f64;
    let energy = latent.iter().map(|value| value * value).sum::<f64>().sqrt();
    let scale = if energy > 1.0 { 1.0 / energy } else { 1.0 };
    let mean_sub = dw[DW_LATENT_START]; // latent_mean_subtraction
    for value in latent.iter_mut() {
        *value = clamp((*value - mean_sub * mean) * scale, -1.0, 1.0);
    }
}

pub fn update_lorenz(lorenz: &mut LorenzState, obs: &Observation, dw: &[f64]) {
    let sigma = LORENZ_SIGMA_BASE + dw[0] * obs.signal - obs.noise;
    let rho = LORENZ_RHO_BASE + dw[1] * (obs.global_score - obs.chaos_risk)
        + dw[2] * obs.route_success;
    let beta = LORENZ_BETA_BASE + dw[3] * obs.memory_pressure;
    let dt = clamp(
        dw[4] + dw[5] * obs.stability + dw[6] * obs.novelty,
        dw[7],
        dw[8],
    );
    let dx = sigma * (lorenz.y - lorenz.x)
        + dw[9] * (obs.signal - obs.noise)
        + dw[10] * (obs.actor_load - obs.actor_stalls);
    let dy = lorenz.x * (rho - lorenz.z) - lorenz.y
        + dw[11] * obs.route_success
        - dw[12] * obs.cost_pressure;
    let dz = lorenz.x * lorenz.y - beta * lorenz.z + dw[13] * obs.graph_density;
    lorenz.x = clamp(lorenz.x + dt * dx, -LORENZ_X_BOUND, LORENZ_X_BOUND);
    lorenz.y = clamp(lorenz.y + dt * dy, -LORENZ_Y_BOUND, LORENZ_Y_BOUND);
    lorenz.z = clamp(lorenz.z + dt * dz, LORENZ_Z_MIN, LORENZ_Z_MAX);
}

pub fn lorenz_energy(lorenz: &LorenzState, dw: &[f64]) -> f64 {
    let norm = dw[DW_LORENZ_AUX_START]; // lorenz_energy_normalization
    clamp(
        (lorenz.x * lorenz.x + lorenz.y * lorenz.y + lorenz.z * lorenz.z).sqrt()
            / norm,
        0.0,
        1.0,
    )
}

pub fn lorenz_basis(lorenz: &LorenzState, row: usize, dw: &[f64]) -> f64 {
    let y_phase = dw[DW_LORENZ_AUX_START + 1]; // lorenz_basis_y_phase
    let z_phase = dw[DW_LORENZ_AUX_START + 2]; // lorenz_basis_z_phase
    let basis_scale = dw[DW_LORENZ_AUX_START + 3]; // lorenz_basis_scale
    let phase = ((row + 1) as f64 * PHI).sin();
    let x = lorenz.x * phase;
    let y = lorenz.y * (phase * y_phase).cos();
    let z = lorenz.z * (phase * z_phase).sin();
    basis_scale * (x + y + z)
}

pub fn cosine_similarity(a: &[f64], b: &[f64]) -> f64 {
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

pub fn hopfield_recall(state: &KernelState) -> ([f64; LATENT_DIM], f64, i64) {
    let dw = &state.dynamic_weights;
    let strength_threshold = dw[DW_HOPFIELD_START];     // hopfield_strength_threshold
    let similarity_base = dw[DW_HOPFIELD_START + 1];    // hopfield_similarity_base
    let strength_scale = dw[DW_HOPFIELD_START + 2];     // hopfield_strength_scale
    let recall_active_threshold = dw[DW_HOPFIELD_START + 3]; // hopfield_recall_active_threshold

    let latent = state.latent.iter().copied().collect::<Vec<_>>();
    let mut best_index = None;
    let mut best_score = 0.0;

    for index in 0..MEMORY_SLOTS {
        let strength = state.memory_strengths[index];
        if strength <= strength_threshold {
            continue;
        }
        let similarity = cosine_similarity(&latent, &state.memory_slots[index]);
        let score = similarity * (similarity_base + strength_scale * strength);
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
        (
            recalled,
            strength,
            if strength > recall_active_threshold { 1 } else { 0 },
        )
    } else {
        (recalled, 0.0, 0)
    }
}

pub fn update_local_weights(
    weights: &mut [f64],
    latent: &[f64; LATENT_DIM],
    target: f64,
    eta: f64,
    dw: &[f64],
) -> f64 {
    let oja_reg = dw[DW_LEARNING_START];     // oja_regularization
    let w_decay = dw[DW_LEARNING_START + 1]; // weight_decay
    let prediction = clamp(dot(weights, latent), -1.0, 1.0);
    let error = target - prediction;
    for (index, weight) in weights.iter_mut().enumerate() {
        let hebbian = error * latent[index];
        let oja = prediction * latent[index] * latent[index] * oja_reg;
        *weight = clamp(
            *weight * w_decay + eta * hebbian - eta * oja,
            -WEIGHT_BOUND,
            WEIGHT_BOUND,
        );
    }
    prediction
}

pub fn head_targets(
    obs: &Observation,
    recall_strength: f64,
    lorenz_e: f64,
    dw: &[f64],
) -> [f64; HEAD_COUNT] {
    let h = DW_HEAD_START;
    let harmony = clamp(
        obs.signal - obs.noise
            + dw[h] * obs.reward                           // harmony_reward_w
            + dw[h + 1] * recall_strength                  // harmony_recall_w
            + dw[h + 2] * lorenz_e                         // harmony_lorenz_w
            + dw[h + 3] * obs.chaos_risk                   // harmony_chaos_w
            + dw[h + 4] * obs.presentation_cleanliness     // harmony_cleanliness_w
            + dw[h + 5] * obs.presentation_user_affinity   // harmony_affinity_w
            + dw[h + 6] * obs.presentation_symbolic_density // harmony_symbolic_w
            + dw[h + 7] * obs.presentation_decor_density,  // harmony_decor_w
        -1.0,
        1.0,
    );
    let routing = clamp(
        obs.route_success
            + dw[h + 8] * obs.cost_pressure      // routing_cost_w
            + dw[h + 9] * obs.route_latency       // routing_latency_w
            + dw[h + 10] * obs.prior_confidence    // routing_confidence_w
            + dw[h + 11] * recall_strength,        // routing_recall_w
        -1.0,
        1.0,
    );
    let memory = clamp(
        (1.0 - obs.memory_pressure) * dw[h + 12]          // memory_pressure_w
            + dw[h + 13] * obs.stability                   // memory_stability_w
            + dw[h + 14] * recall_strength                 // memory_recall_w
            + dw[h + 15] * obs.field_recall_strength       // memory_field_recall_w
            + dw[h + 16] * obs.field_basin_stability       // memory_basin_w
            + dw[h + 17] * obs.field_eigenmode_coherence   // memory_eigenmode_w
            + dw[h + 18] * obs.presentation_user_affinity  // memory_affinity_w
            + dw[h + 19] * obs.presentation_verbosity,     // memory_verbosity_w
        -1.0,
        1.0,
    );
    let evolution = clamp(
        obs.signal - obs.chaos_risk
            + if obs.rewrite_ready {
                dw[h + 20]  // evolution_rewrite_ready_bonus
            } else {
                dw[h + 21]  // evolution_rewrite_ready_penalty
            }
            + dw[h + 22] * obs.stability                  // evolution_stability_w
            + dw[h + 23] * obs.presentation_cleanliness,   // evolution_cleanliness_w
        -1.0,
        1.0,
    );
    let security = clamp(
        1.0 - posture_scalar(obs.security_posture.trim())
            + dw[h + 24] * obs.noise                      // security_noise_w
            + dw[h + 25] * obs.error_pressure              // security_error_w
            + dw[h + 26] * obs.presentation_decor_density, // security_decor_w
        -1.0,
        1.0,
    );
    [harmony, routing, memory, evolution, security]
}

pub fn build_projection(
    state: &KernelState,
    obs: &Observation,
    predictions: [f64; HEAD_COUNT],
    recall_strength: f64,
) -> Projection {
    let dw = &state.dynamic_weights;
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

    // Confidence weights (indices 29..35)
    let c = DW_CONFIDENCE_START;
    let confidence = clamp(
        dw[c] * obs.stability                          // confidence_stability_w
            + dw[c + 1] * (1.0 - obs.chaos_risk)      // confidence_antichaos_w
            + dw[c + 2] * (1.0 - obs.noise)            // confidence_antinoise_w
            + dw[c + 3] * recall_strength               // confidence_recall_w
            + dw[c + 4] * lorenz_energy(&state.lorenz, dw) // confidence_lorenz_w
            + dw[c + 5] * obs.presentation_cleanliness, // confidence_cleanliness_w
        0.0,
        1.0,
    );

    // Projection scales (indices 62..78)
    let s = DW_PROJ_SCALE_START;
    let hs_scale = dw[s];
    let hn_scale = dw[s + 1];
    let rs_scale = dw[s + 2];
    let rc_scale = dw[s + 3];
    let ea_scale = dw[s + 4];
    let rp_scale = dw[s + 5];
    let rsp_scale = dw[s + 6];
    let rsu_scale = dw[s + 7];
    let mc_scale = dw[s + 8];
    let sd_scale = dw[s + 9];
    let sa_scale = dw[s + 10];
    let pv_scale = dw[s + 11];
    let pm_scale = dw[s + 12];
    let ps_scale = dw[s + 13];
    let psr_scale = dw[s + 14];
    let pd_scale = dw[s + 15];

    // Projection alphas (indices 78..89)
    let a = DW_PROJ_ALPHA_START;
    let hs_alpha = dw[a];
    let hn_alpha = dw[a + 1];
    let rs_alpha = dw[a + 2];
    let rc_alpha = dw[a + 3];
    let ea_alpha = dw[a + 4];
    let rp_alpha = dw[a + 5];
    let rsp_alpha = dw[a + 6];
    let rsu_alpha = dw[a + 7];
    let mc_alpha = dw[a + 8];
    let sd_alpha = dw[a + 9];
    let sa_alpha = dw[a + 10];

    // Routing reasoning (indices 89..93)
    let rr = DW_ROUTING_REASONING_START;
    let rr_routing_w = dw[rr];
    let rr_harmony_w = dw[rr + 1];
    let rr_recall_w = dw[rr + 2];
    let rr_scale = dw[rr + 3];

    // Routing vitruvian (indices 93..95)
    let rv = DW_ROUTING_VITRUVIAN_START;
    let rv_alpha = dw[rv];
    let rv_scale = dw[rv + 1];

    // Memory recall limit (indices 95..98)
    let mr = DW_MEMORY_RECALL_LIMIT_START;
    let mr_head_scale = dw[mr];
    let mr_strength_scale = dw[mr + 1];
    let mr_limit_bound = dw[mr + 2];

    // Presentation mixing (indices 98..116)
    let p = DW_PRES_MIX_START;

    Projection {
        cycle: obs.cycle,
        confidence,
        stability: clamp(obs.stability, 0.0, 1.0),
        novelty: clamp(obs.novelty, 0.0, 1.0),
        latent_energy,
        recall_strength,
        harmony_signal_bias: clamp(
            (hs_alpha * harmony_head
                + (1.0 - hs_alpha) * recall_strength)
                * hs_scale,
            -hs_scale,
            hs_scale,
        ),
        harmony_noise_bias: clamp(
            (-hn_alpha * harmony_head
                - (1.0 - hn_alpha) * recall_strength)
                * hn_scale,
            -hn_scale,
            hn_scale,
        ),
        rewrite_signal_delta: clamp(
            (rs_alpha * harmony_head
                + (1.0 - rs_alpha) * recall_strength)
                * rs_scale,
            -rs_scale,
            rs_scale,
        ),
        rewrite_chaos_delta: clamp(
            (-rc_alpha * evolution_head
                - (1.0 - rc_alpha) * recall_strength)
                * rc_scale,
            -rc_scale,
            rc_scale,
        ),
        evolution_aggression_bias: clamp(
            (ea_alpha * evolution_head
                + (1.0 - ea_alpha) * lorenz_energy(&state.lorenz, dw))
                * ea_scale,
            -ea_scale,
            ea_scale,
        ),
        // Logistic-r delta: same evolution head, anti-chaos auxiliary so the kernel
        // pushes r toward the edge under low chaos and pulls it back under high chaos.
        // Same scale envelope as aggression_bias; consumer applies a tighter policy clamp.
        logistic_r_delta: clamp(
            (ea_alpha * evolution_head
                + (1.0 - ea_alpha) * (1.0 - obs.chaos_risk))
                * ea_scale,
            -ea_scale,
            ea_scale,
        ),
        routing_price_delta: clamp(
            (-rp_alpha * routing_head
                - (1.0 - rp_alpha) * obs.cost_pressure)
                * rp_scale,
            -rp_scale,
            rp_scale,
        ),
        routing_speed_delta: clamp(
            (rsp_alpha * routing_head
                + (1.0 - rsp_alpha) * recall_strength)
                * rsp_scale,
            -rsp_scale,
            rsp_scale,
        ),
        routing_success_delta: clamp(
            (rsu_alpha * routing_head
                + (1.0 - rsu_alpha) * obs.route_success)
                * rsu_scale,
            -rsu_scale,
            rsu_scale,
        ),
        routing_reasoning_delta: clamp(
            rr_routing_w * routing_head
                + rr_harmony_w * harmony_head
                + rr_recall_w * recall_strength,
            -rr_scale,
            rr_scale,
        ),
        routing_vitruvian_min_delta: clamp(
            (rv_alpha * harmony_head
                + (1.0 - rv_alpha) * recall_strength)
                * rv_scale,
            -rv_scale,
            rv_scale,
        ),
        memory_recall_limit_delta: clamp(
            memory_head * mr_head_scale
                + recall_strength * mr_strength_scale,
            -mr_limit_bound,
            mr_limit_bound,
        )
        .round() as i32,
        memory_crystal_threshold_delta: clamp(
            (mc_alpha * memory_head
                + (1.0 - mc_alpha) * recall_strength)
                * mc_scale,
            -mc_scale,
            mc_scale,
        ),
        security_dissonance_delta: clamp(
            (-sd_alpha * security_head
                - (1.0 - sd_alpha) * obs.error_pressure)
                * sd_scale,
            -sd_scale,
            sd_scale,
        ),
        security_anomaly_delta: clamp(
            (sa_alpha * security_head
                - (1.0 - sa_alpha) * obs.error_pressure)
                * sa_scale,
            -sa_scale,
            sa_scale,
        ),
        presentation_verbosity_delta: clamp(
            (dw[p] * memory_head             // pres_verbosity_memory_w
                + dw[p + 1] * obs.presentation_verbosity  // pres_verbosity_current_w
                + dw[p + 2] * affinity            // pres_verbosity_affinity_w
                + dw[p + 3] * cleanliness)        // pres_verbosity_clean_w
                * pv_scale,
            -pv_scale,
            pv_scale,
        ),
        presentation_markdown_density_delta: clamp(
            (dw[p + 4] * memory_head             // pres_markdown_memory_w
                + dw[p + 5] * obs.presentation_markdown_density // pres_markdown_current_w
                + dw[p + 6] * affinity)           // pres_markdown_affinity_w
                * pm_scale,
            -pm_scale,
            pm_scale,
        ),
        presentation_symbolic_density_delta: clamp(
            (dw[p + 7] * obs.presentation_symbolic_density  // pres_symbolic_current_w
                + dw[p + 8] * harmony_head        // pres_symbolic_harmony_w
                + dw[p + 9] * recall_strength     // pres_symbolic_recall_w
                + dw[p + 10] * cleanliness)       // pres_symbolic_clean_w
                * ps_scale,
            -ps_scale,
            ps_scale,
        ),
        presentation_self_reference_delta: clamp(
            (dw[p + 11] * obs.presentation_self_reference // pres_selfref_current_w
                + dw[p + 12] * harmony_head       // pres_selfref_harmony_w
                + dw[p + 13] * affinity           // pres_selfref_affinity_w
                + dw[p + 14] * cleanliness)       // pres_selfref_clean_w
                * psr_scale,
            -psr_scale,
            psr_scale,
        ),
        presentation_decor_density_delta: clamp(
            (dw[p + 15] * obs.presentation_decor_density  // pres_decor_current_w
                + dw[p + 16] * (1.0 - obs.presentation_cleanliness) // pres_decor_unclean_w
                + dw[p + 17] * harmony_head)      // pres_decor_harmony_w
                * pd_scale,
            -pd_scale,
            pd_scale,
        ),
    }
}

pub fn step_kernel(state: &mut KernelState, obs: &Observation) -> Projection {
    let dw = &state.dynamic_weights;
    update_lorenz(&mut state.lorenz, obs, dw);
    let input = observation_vector(obs);
    let (recalled, recall_strength, _) = hopfield_recall(state);

    let dw = &state.dynamic_weights;
    let recurrence = dw[DW_LATENT_START + 1]; // latent_recurrence_coeff
    let recall_coupling = dw[DW_LATENT_START + 2]; // latent_recall_coupling

    let mut next_latent = [0.0; LATENT_DIM];
    for row in 0..LATENT_DIM {
        let mut acc = recurrence * state.latent[row];
        for (col, value) in input.iter().enumerate() {
            acc += state.input_matrix[row * INPUT_DIM + col] * *value;
        }
        for col in 0..LATENT_DIM {
            acc += state.recurrent_matrix[row * LATENT_DIM + col] * state.latent[col];
        }
        acc += recall_coupling * recalled[row];
        acc += lorenz_basis(&state.lorenz, row, dw);
        next_latent[row] = clamp(acc.tanh(), -1.0, 1.0);
    }

    normalize_latent(&mut next_latent, dw);
    state.latent = next_latent;
    state.cycle = obs.cycle;
    state.last_observation = obs.clone();
    state.last_recall_strength = recall_strength;

    let mem_usage_decay = state.dynamic_weights[DW_LEARNING_START + 3]; // memory_usage_decay
    for usage in state.memory_usage.iter_mut() {
        *usage *= mem_usage_decay;
    }

    let dw = &state.dynamic_weights;
    let le = lorenz_energy(&state.lorenz, dw);
    let targets = head_targets(obs, recall_strength, le, dw);
    let lr = dw[DW_LEARNING_START + 2]; // learning_rate
    let mut predictions = [0.0; HEAD_COUNT];
    for (index, head) in state.readout_weights.iter_mut().enumerate() {
        predictions[index] = update_local_weights(head, &state.latent, targets[index], lr, dw);
    }

    let projection = build_projection(state, obs, predictions, recall_strength);

    // Hebbian update of dynamic weights based on feedback confidence error.
    let confidence_error = state.last_feedback.applied_confidence - projection.confidence;
    let eta_dw = weights::INIT_DYNAMIC_LEARNING_RATE;
    let le = lorenz_energy(&state.lorenz, &state.dynamic_weights);
    for i in 0..weights::DYNAMIC_WEIGHT_COUNT {
        let signal = if i < DW_LORENZ_START + DW_LORENZ_COUNT + DW_LORENZ_AUX_COUNT {
            le  // Lorenz weights learn from energy
        } else if i < DW_HOPFIELD_START + DW_HOPFIELD_COUNT {
            recall_strength  // Hopfield weights learn from recall
        } else if i < DW_HEAD_START + DW_HEAD_COUNT {
            recall_strength  // Head weights learn from recall
        } else {
            predictions[i % HEAD_COUNT]  // Projection weights learn from head predictions
        };
        let dw_val = state.dynamic_weights[i];
        // Safe clamp bounds: don't let weights drift more than 2x from current,
        // and prevent zero-crossing for weights that must stay positive or negative.
        let (lo, hi) = if dw_val > 0.0 {
            (dw_val * 0.5, dw_val * 2.0)
        } else if dw_val < 0.0 {
            (dw_val * 2.0, dw_val * 0.5)
        } else {
            (-0.01, 0.01)  // Allow tiny drift from zero
        };
        state.dynamic_weights[i] = clamp(
            dw_val + eta_dw * confidence_error * signal,
            lo,
            hi,
        );
    }

    state.last_projection = projection.clone();
    projection
}
