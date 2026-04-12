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

// ── Service trait: Free Monad pattern for dispatch ───────────────────
//
// handle(&self, cmd) → (Delta, Ok)  — PURE, no mutation
// apply(&mut self, delta)            — single mutation point

use std::path::PathBuf;

/// Command enum — pure data describing what to do.
pub enum SignalogradCmd {
    Observe(Observation),
    ApplyFeedback(Feedback),
    Status,
    Snapshot,
    Reset,
    Checkpoint(PathBuf),
    Restore(PathBuf),
    SaveToDisk,
}

/// Delta — describes how state changes. Inspectable, composable.
pub enum SignalogradDelta {
    Observed { projection: Projection },
    FeedbackApplied { feedback: Feedback },
    StateReplaced(Box<KernelState>),
    Saved,
    Noop,
}

/// Result — what each command produces.
pub enum SignalogradOk {
    Ok,
    Status(String),
    Snapshot(String),
}

impl SignalogradOk {
    pub fn to_sexp(&self) -> String {
        match self {
            SignalogradOk::Ok => "(:ok)".to_string(),
            SignalogradOk::Status(s) => format!(
                "(:ok :result \"{}\")",
                harmonia_actor_protocol::sexp_escape(s)
            ),
            SignalogradOk::Snapshot(s) => format!(
                "(:ok :result \"{}\")",
                harmonia_actor_protocol::sexp_escape(s)
            ),
        }
    }
}

impl harmonia_actor_protocol::Service for KernelState {
    type Cmd = SignalogradCmd;
    type Ok = SignalogradOk;
    type Delta = SignalogradDelta;

    fn handle(&self, cmd: Self::Cmd) -> Result<(Self::Delta, Self::Ok), harmonia_actor_protocol::MemoryError> {
        match cmd {
            SignalogradCmd::Observe(observation) => {
                // Clone state for pure computation, step kernel, return delta.
                let mut scratch = self.clone();
                let _projection = step_kernel(&mut scratch, &observation);
                scratch.checkpoint_digest = simple_hash(&state_to_sexp(&scratch));
                Ok((
                    SignalogradDelta::StateReplaced(Box::new(scratch)),
                    SignalogradOk::Ok,
                ))
            }
            SignalogradCmd::ApplyFeedback(feedback) => {
                let mut scratch = self.clone();
                apply_feedback(&mut scratch, &feedback);
                scratch.checkpoint_digest = simple_hash(&state_to_sexp(&scratch));
                Ok((
                    SignalogradDelta::StateReplaced(Box::new(scratch)),
                    SignalogradOk::Ok,
                ))
            }
            SignalogradCmd::Status => {
                Ok((SignalogradDelta::Noop, SignalogradOk::Status(status_sexp(self))))
            }
            SignalogradCmd::Snapshot => {
                Ok((SignalogradDelta::Noop, SignalogradOk::Snapshot(snapshot_sexp(self))))
            }
            SignalogradCmd::Reset => {
                let mut fresh = KernelState::new();
                fresh.checkpoint_digest = simple_hash(&state_to_sexp(&fresh));
                Ok((
                    SignalogradDelta::StateReplaced(Box::new(fresh)),
                    SignalogradOk::Ok,
                ))
            }
            SignalogradCmd::Checkpoint(path) => {
                write_state_to_path(self, &path)
                    .map_err(|e| harmonia_actor_protocol::MemoryError::PersistenceFailed(e))?;
                Ok((SignalogradDelta::Saved, SignalogradOk::Ok))
            }
            SignalogradCmd::Restore(path) => {
                let restored = restore_state_from_path(&path)
                    .map_err(|e| harmonia_actor_protocol::MemoryError::PersistenceFailed(e))?;
                Ok((
                    SignalogradDelta::StateReplaced(Box::new(restored)),
                    SignalogradOk::Ok,
                ))
            }
            SignalogradCmd::SaveToDisk => {
                save_state(self)
                    .map_err(|e| harmonia_actor_protocol::MemoryError::PersistenceFailed(e))?;
                Ok((SignalogradDelta::Saved, SignalogradOk::Ok))
            }
        }
    }

    fn apply(&mut self, delta: Self::Delta) {
        match delta {
            SignalogradDelta::StateReplaced(new_state) => *self = *new_state,
            SignalogradDelta::Observed { .. }
            | SignalogradDelta::FeedbackApplied { .. }
            | SignalogradDelta::Saved
            | SignalogradDelta::Noop => {}
        }
    }
}

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
