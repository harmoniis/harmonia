use crate::error::clamp;
use crate::model::{
    KernelState, LorenzState, Observation, Projection, HEAD_COUNT, INPUT_DIM, LATENT_DIM,
    MEMORY_SLOTS, PHI,
};
use crate::observation::{observation_vector, posture_scalar};

pub fn dot(weights: &[f64], latent: &[f64; LATENT_DIM]) -> f64 {
    weights
        .iter()
        .zip(latent.iter())
        .map(|(w, l)| *w * *l)
        .sum::<f64>()
        / LATENT_DIM as f64
}

pub fn normalize_latent(latent: &mut [f64; LATENT_DIM]) {
    let mean = latent.iter().sum::<f64>() / LATENT_DIM as f64;
    let energy = latent.iter().map(|value| value * value).sum::<f64>().sqrt();
    let scale = if energy > 1.0 { 1.0 / energy } else { 1.0 };
    for value in latent.iter_mut() {
        *value = clamp((*value - 0.12 * mean) * scale, -1.0, 1.0);
    }
}

pub fn update_lorenz(lorenz: &mut LorenzState, obs: &Observation) {
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

pub fn lorenz_energy(lorenz: &LorenzState) -> f64 {
    clamp(
        (lorenz.x * lorenz.x + lorenz.y * lorenz.y + lorenz.z * lorenz.z).sqrt() / 40.0,
        0.0,
        1.0,
    )
}

pub fn lorenz_basis(lorenz: &LorenzState, row: usize) -> f64 {
    let phase = ((row + 1) as f64 * PHI).sin();
    let x = lorenz.x * phase;
    let y = lorenz.y * (phase * 0.7).cos();
    let z = lorenz.z * (phase * 0.3).sin();
    0.018 * (x + y + z)
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

pub fn update_local_weights(
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

pub fn head_targets(
    obs: &Observation,
    recall_strength: f64,
    lorenz_energy: f64,
) -> [f64; HEAD_COUNT] {
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
        (1.0 - obs.memory_pressure) * 0.40
            + 0.18 * obs.stability
            + 0.10 * recall_strength
            + 0.10 * obs.field_recall_strength
            + 0.08 * obs.field_basin_stability
            + 0.07 * obs.field_eigenmode_coherence
            + 0.07 * obs.presentation_user_affinity
            - 0.08 * obs.presentation_verbosity,
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

pub fn build_projection(
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

pub fn step_kernel(state: &mut KernelState, obs: &Observation) -> Projection {
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
