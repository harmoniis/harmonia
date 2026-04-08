/// Session actor dispatch — sexp command routing for the session component.
///
/// The session actor owns `SessionState` and dispatches all session
/// operations through this module. Pure functional: sexp in, sexp out.

use std::path::PathBuf;

use crate::sessions::{
    self, Session, SessionState,
};

/// Dispatch a session command (sexp in, sexp out).
pub fn dispatch(state: &mut SessionState, sexp: &str) -> String {
    let op = harmonia_actor_protocol::extract_sexp_string(sexp, ":op")
        .unwrap_or_default();
    match op.as_str() {
        "session-create" => dispatch_create(state, sexp),
        "session-list" => dispatch_list(sexp),
        "session-resume" => dispatch_resume(state, sexp),
        "session-current" => dispatch_current(state),
        "session-events" => dispatch_events(state, sexp),
        "session-append" => dispatch_append(state, sexp),
        _ => format!(
            "(:error \"unknown session op: {}\")",
            harmonia_actor_protocol::sexp_escape(&op)
        ),
    }
}

fn resolve_or_error() -> Result<(PathBuf, String), String> {
    let data_dir = sessions::resolve_data_dir()?;
    let label = sessions::resolve_node_label()?;
    Ok((data_dir, label))
}

fn ok_json(json: &str) -> String {
    format!("(:ok :json \"{}\")", harmonia_actor_protocol::sexp_escape(json))
}

fn err_sexp(e: &str) -> String {
    format!("(:error \"{}\")", harmonia_actor_protocol::sexp_escape(e))
}

fn dispatch_create(state: &mut SessionState, sexp: &str) -> String {
    let (data_dir, label) = match resolve_or_error() {
        Ok(v) => v,
        Err(e) => return err_sexp(&e),
    };
    let label = harmonia_actor_protocol::extract_sexp_string(sexp, ":node-label")
        .filter(|s| !s.is_empty())
        .unwrap_or(label);
    match sessions::create(&label, &data_dir) {
        Ok(s) => {
            let json = serde_json::to_string(&s).unwrap_or_default();
            state.set_current(s);
            ok_json(&json)
        }
        Err(e) => err_sexp(&e),
    }
}

fn dispatch_list(sexp: &str) -> String {
    let (data_dir, label) = match resolve_or_error() {
        Ok(v) => v,
        Err(e) => return err_sexp(&e),
    };
    let label = harmonia_actor_protocol::extract_sexp_string(sexp, ":node-label")
        .filter(|s| !s.is_empty())
        .unwrap_or(label);
    match sessions::list(&label, &data_dir) {
        Ok(summaries) => {
            let json = serde_json::to_string(&summaries).unwrap_or_default();
            ok_json(&json)
        }
        Err(e) => err_sexp(&e),
    }
}

fn dispatch_resume(state: &mut SessionState, sexp: &str) -> String {
    let (data_dir, label) = match resolve_or_error() {
        Ok(v) => v,
        Err(e) => return err_sexp(&e),
    };
    let session_id = harmonia_actor_protocol::extract_sexp_string(sexp, ":session-id")
        .unwrap_or_default();
    if session_id.is_empty() {
        return "(:error \"missing :session-id\")".to_string();
    }
    match sessions::resume(&label, &data_dir, &session_id) {
        Ok(s) => {
            let json = serde_json::to_string(&s).unwrap_or_default();
            state.set_current(s);
            ok_json(&json)
        }
        Err(e) => err_sexp(&e),
    }
}

fn dispatch_current(state: &mut SessionState) -> String {
    let (data_dir, label) = match resolve_or_error() {
        Ok(v) => v,
        Err(e) => return err_sexp(&e),
    };
    if let Some(s) = state.current_ref() {
        let json = serde_json::to_string(s).unwrap_or_default();
        return ok_json(&json);
    }
    match sessions::current(&label, &data_dir) {
        Ok(Some(s)) => {
            let json = serde_json::to_string(&s).unwrap_or_default();
            state.set_current(s);
            ok_json(&json)
        }
        Ok(None) => "(:ok :json \"null\")".to_string(),
        Err(e) => err_sexp(&e),
    }
}

fn dispatch_events(state: &mut SessionState, sexp: &str) -> String {
    let (data_dir, label) = match resolve_or_error() {
        Ok(v) => v,
        Err(e) => return err_sexp(&e),
    };
    let session_id = harmonia_actor_protocol::extract_sexp_string(sexp, ":session-id")
        .unwrap_or_default();
    let session = if session_id.is_empty() {
        resolve_current(state, &label, &data_dir)
    } else {
        sessions::resume(&label, &data_dir, &session_id).ok()
    };
    let session = match session {
        Some(s) => s,
        None => return "(:ok :json \"[]\")".to_string(),
    };
    match sessions::read_events(&session) {
        Ok(events) => {
            let json = serde_json::to_string(&events).unwrap_or_default();
            ok_json(&json)
        }
        Err(e) => err_sexp(&e),
    }
}

fn dispatch_append(state: &mut SessionState, sexp: &str) -> String {
    let (data_dir, label) = match resolve_or_error() {
        Ok(v) => v,
        Err(e) => return err_sexp(&e),
    };
    let actor_name = harmonia_actor_protocol::extract_sexp_string(sexp, ":actor")
        .unwrap_or_else(|| "system".to_string());
    let kind = harmonia_actor_protocol::extract_sexp_string(sexp, ":kind")
        .unwrap_or_else(|| "event".to_string());
    let text = harmonia_actor_protocol::extract_sexp_string(sexp, ":text")
        .unwrap_or_default();
    let session = match resolve_current(state, &label, &data_dir) {
        Some(s) => s,
        None => return "(:error \"no active session\")".to_string(),
    };
    match sessions::append_event(&session, &actor_name, &kind, &text) {
        Ok(()) => "(:ok)".to_string(),
        Err(e) => err_sexp(&e),
    }
}

/// Resolve the current session from state cache or disk.
fn resolve_current(
    state: &mut SessionState,
    label: &str,
    data_dir: &std::path::Path,
) -> Option<Session> {
    if let Some(s) = state.current_ref() {
        return Some(s.clone());
    }
    if let Ok(Some(s)) = sessions::current(label, data_dir) {
        state.set_current(s.clone());
        Some(s)
    } else {
        None
    }
}
