//! Signalograd component dispatch — requires actor-owned KernelState.

use super::param;

pub(crate) fn dispatch(
    sexp: &str,
    state: &mut harmonia_signalograd::KernelState,
) -> String {
    use harmonia_signalograd::{
        apply_feedback, parse_feedback, parse_observation, restore_state_from_path, save_state,
        simple_hash, snapshot_sexp, state_to_sexp, status_sexp, step_kernel, write_state_to_path,
        KernelState,
    };
    use std::path::PathBuf;

    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();
    match op.as_str() {
        "init" => "(:ok)".to_string(),
        "observe" => {
            let raw = param!(sexp, ":observation");
            let observation = match parse_observation(&raw) {
                Ok(o) => o,
                Err(e) => return format!("(:error \"observe parse: {e}\")"),
            };
            let _projection = step_kernel(state, &observation);
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"observe save: {e}\")");
            }
            "(:ok)".to_string()
        }
        "status" => format!("(:ok :result \"{}\")", harmonia_actor_protocol::sexp_escape(&status_sexp(state))),
        "snapshot" => format!("(:ok :result \"{}\")", harmonia_actor_protocol::sexp_escape(&snapshot_sexp(state))),
        "feedback" => {
            let raw = param!(sexp, ":feedback");
            let feedback = match parse_feedback(&raw) {
                Ok(f) => f,
                Err(e) => return format!("(:error \"feedback parse: {e}\")"),
            };
            apply_feedback(state, &feedback);
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"feedback save: {e}\")");
            }
            "(:ok)".to_string()
        }
        "reset" => {
            *state = KernelState::new();
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"reset save: {e}\")");
            }
            "(:ok)".to_string()
        }
        "checkpoint" => {
            let path_str = param!(sexp, ":path");
            let target = PathBuf::from(path_str.trim());
            if let Err(e) = write_state_to_path(state, &target) {
                return format!("(:error \"checkpoint failed: {e}\")");
            }
            state.checkpoint_digest = simple_hash(&state_to_sexp(state));
            if let Err(e) = save_state(state) {
                return format!("(:error \"checkpoint save: {e}\")");
            }
            "(:ok)".to_string()
        }
        "restore" => {
            let path_str = param!(sexp, ":path");
            let target = PathBuf::from(path_str.trim());
            match restore_state_from_path(&target) {
                Ok(restored) => {
                    *state = restored;
                    state.checkpoint_digest = simple_hash(&state_to_sexp(state));
                    if let Err(e) = save_state(state) {
                        return format!("(:error \"restore save: {e}\")");
                    }
                    "(:ok)".to_string()
                }
                Err(e) => format!("(:error \"restore failed: {e}\")"),
            }
        }
        _ => format!("(:error \"unknown signalograd op: {}\")", op),
    }
}
