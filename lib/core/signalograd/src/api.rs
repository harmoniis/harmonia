use std::ffi::CString;
use std::fs;
use std::os::raw::c_char;
use std::path::PathBuf;

use crate::checkpoint::{restore_state_from_path, save_state, state_to_sexp};
use crate::error::{
    clear_error, cstr_to_string, last_error_message, set_error, simple_hash, state, to_c_string,
};
use crate::feedback::apply_feedback;
use crate::format::{snapshot_sexp, status_sexp};
use crate::kernel::step_kernel;
use crate::model::{KernelState, Projection, VERSION};
use crate::observation::{parse_feedback, parse_observation};

fn ensure_actor_registered() {
    // Actor registration is now handled by the runtime IPC system.
}

fn post_projection_signal(_projection: &Projection) {
    // Actor mailbox posting is now handled by the runtime IPC system.
}

pub fn harmonia_signalograd_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

pub fn harmonia_signalograd_healthcheck() -> i32 {
    1
}

pub fn harmonia_signalograd_init() -> i32 {
    let _ = state();
    ensure_actor_registered();
    clear_error();
    0
}

pub fn harmonia_signalograd_observe(observation_sexp: *const c_char) -> i32 {
    let raw = match cstr_to_string(observation_sexp) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let observation = match parse_observation(&raw) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };

    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };

    let projection = step_kernel(&mut state, &observation);
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    post_projection_signal(&projection);
    clear_error();
    0
}

pub fn harmonia_signalograd_reflect(observation_json: *const c_char) -> i32 {
    harmonia_signalograd_observe(observation_json)
}

pub fn harmonia_signalograd_feedback(feedback_sexp: *const c_char) -> i32 {
    let raw = match cstr_to_string(feedback_sexp) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let feedback = match parse_feedback(&raw) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };

    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };

    apply_feedback(&mut state, &feedback);
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

pub fn harmonia_signalograd_checkpoint(path: *const c_char) -> i32 {
    let raw_path = match cstr_to_string(path) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let target = PathBuf::from(raw_path.trim());
    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };

    let body = state_to_sexp(&state);
    state.checkpoint_digest = simple_hash(&body);
    if let Some(parent) = target.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            set_error(err.to_string());
            return -1;
        }
    }
    if let Err(err) = fs::write(&target, body).map_err(|e| e.to_string()) {
        set_error(err);
        return -1;
    }
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

pub fn harmonia_signalograd_restore(path: *const c_char) -> i32 {
    let raw_path = match cstr_to_string(path) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };
    let target = PathBuf::from(raw_path.trim());
    let restored = match restore_state_from_path(&target) {
        Ok(value) => value,
        Err(err) => {
            set_error(err);
            return -1;
        }
    };

    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };
    *state = restored;
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

pub fn harmonia_signalograd_status() -> *mut c_char {
    let state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return std::ptr::null_mut();
        }
    };
    clear_error();
    to_c_string(status_sexp(&state))
}

pub fn harmonia_signalograd_snapshot() -> *mut c_char {
    let state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return std::ptr::null_mut();
        }
    };
    clear_error();
    to_c_string(snapshot_sexp(&state))
}

pub fn harmonia_signalograd_reset() -> i32 {
    let mut state = match state().lock() {
        Ok(value) => value,
        Err(_) => {
            set_error("signalograd state lock poisoned");
            return -1;
        }
    };
    *state = KernelState::new();
    state.checkpoint_digest = simple_hash(&state_to_sexp(&state));
    if let Err(err) = save_state(&state) {
        set_error(err);
        return -1;
    }
    clear_error();
    0
}

pub fn harmonia_signalograd_last_error() -> *mut c_char {
    to_c_string(last_error_message())
}

pub fn harmonia_signalograd_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
