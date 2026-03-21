use crate::error::clamp;
use crate::kernel::{cosine_similarity, update_local_weights};
use crate::model::{Feedback, KernelState, Observation, HEAD_COUNT, MEMORY_SLOTS};

pub(crate) fn feedback_targets(
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

pub(crate) fn remember_state(state: &mut KernelState, feedback: &Feedback) {
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

pub(crate) fn apply_feedback(state: &mut KernelState, feedback: &Feedback) {
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
