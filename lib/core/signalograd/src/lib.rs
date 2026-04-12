mod api; // emptied — C FFI layer removed
pub mod checkpoint;
mod error;
pub mod feedback;
pub mod format;
pub mod kernel;
pub mod model;
pub mod observation;
mod sexp;
pub mod weights;

// ── Typed API: actor-owned state, no singletons ──────────────────────
// The runtime's SignalogradActor owns KernelState and calls these directly.
pub use checkpoint::{restore_state_from_path, save_state, state_to_sexp, write_state_to_path};
pub use error::simple_hash;
pub use feedback::apply_feedback;
pub use format::{projection_to_sexp, snapshot_sexp, status_sexp};
pub use kernel::step_kernel;
pub use model::{Feedback, KernelState, Observation, Projection};
pub use observation::{parse_feedback, parse_observation};

#[cfg(test)]
mod tests {
    use crate::checkpoint::restore_state_from_path;
    use crate::checkpoint::write_state_to_path;
    use crate::feedback::apply_feedback;
    use crate::kernel::step_kernel;
    use crate::model::{Feedback, KernelState, Observation, MEMORY_SLOTS};
    use crate::observation::parse_observation;

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
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir_all(dir);
    }
}
