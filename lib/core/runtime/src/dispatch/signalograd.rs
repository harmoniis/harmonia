//! Signalograd component dispatch — Service pattern (handle → delta → apply).
//!
//! Parse sexp -> SignalogradCmd -> handle(&self) -> (Delta, Ok) -> apply(&mut self) -> to_sexp()

use super::param;

pub(crate) fn dispatch(
    sexp: &str,
    state: &mut harmonia_signalograd::KernelState,
) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op").unwrap_or_default();

    // Parse command from sexp.
    let cmd = match parse_command(sexp, &op) {
        Some(cmd) => cmd,
        None => return format!("(:error \"unknown signalograd op: {}\")", op),
    };

    // Handle + apply + serialize (Service pattern).
    use harmonia_actor_protocol::Service;
    match state.handle(cmd) {
        Ok((delta, result)) => {
            state.apply(delta);
            // Persist after state-mutating operations.
            if is_mutating(&op) {
                if let Err(e) = harmonia_signalograd::save_state(state) {
                    return format!("(:error \"{} save: {e}\")", op);
                }
            }
            result.to_sexp()
        }
        Err(e) => {
            let msg = e.to_string();
            format!("(:error \"{}: {}\")", op, harmonia_actor_protocol::sexp_escape(&msg))
        }
    }
}

fn is_mutating(op: &str) -> bool {
    matches!(op, "observe" | "feedback" | "reset" | "checkpoint" | "restore")
}

fn parse_command(sexp: &str, op: &str) -> Option<harmonia_signalograd::SignalogradCmd> {
    use harmonia_signalograd::SignalogradCmd;
    match op {
        "init" => Some(SignalogradCmd::Status), // init is a no-op, return status
        "observe" => {
            let raw = param!(sexp, ":observation");
            match harmonia_signalograd::parse_observation(&raw) {
                Ok(obs) => Some(SignalogradCmd::Observe(obs)),
                Err(_) => None,
            }
        }
        "feedback" => {
            let raw = param!(sexp, ":feedback");
            match harmonia_signalograd::parse_feedback(&raw) {
                Ok(fb) => Some(SignalogradCmd::ApplyFeedback(fb)),
                Err(_) => None,
            }
        }
        "status" => Some(SignalogradCmd::Status),
        "snapshot" => Some(SignalogradCmd::Snapshot),
        "reset" => Some(SignalogradCmd::Reset),
        "checkpoint" => {
            let path_str = param!(sexp, ":path");
            Some(SignalogradCmd::Checkpoint(std::path::PathBuf::from(path_str.trim())))
        }
        "restore" => {
            let path_str = param!(sexp, ":path");
            Some(SignalogradCmd::Restore(std::path::PathBuf::from(path_str.trim())))
        }
        "save" => Some(SignalogradCmd::SaveToDisk),
        _ => None,
    }
}
