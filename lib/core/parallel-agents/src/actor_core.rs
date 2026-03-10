//! Unified actor registry — the ONE global instance.
//!
//! This module owns the `static REGISTRY` and exports all `harmonia_actor_*`
//! FFI functions. Other crates call these via `dlsym` through the
//! `harmonia_actor_protocol::client` module.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

// Re-export all types from actor-protocol for internal use
pub use harmonia_actor_protocol::*;

// ─── Global registry (the ONE instance) ─────────────────────────────────

static REGISTRY: OnceLock<RwLock<ActorRegistry>> = OnceLock::new();

pub fn registry() -> &'static RwLock<ActorRegistry> {
    REGISTRY.get_or_init(|| RwLock::new(ActorRegistry::new()))
}

// ─── FFI helpers ────────────────────────────────────────────────────────

fn cstr_to_string_local(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string_local(value: String) -> *mut c_char {
    CString::new(value)
        .map(|v| v.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

// ─── FFI exports ────────────────────────────────────────────────────────

/// Register an actor of the given kind. Returns actor ID (>= 1) or -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_actor_register(kind: *const c_char) -> i64 {
    let kind_str = match cstr_to_string_local(kind) {
        Ok(v) => v,
        Err(_) => return -1,
    };
    let actor_kind = match ActorKind::from_str(&kind_str) {
        Ok(v) => v,
        Err(_) => return -1,
    };
    match registry().write() {
        Ok(mut reg) => reg.register(actor_kind) as i64,
        Err(_) => -1,
    }
}

/// Report progress heartbeat for an actor. Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_actor_heartbeat(actor_id: i64, bytes_delta: u64) -> i32 {
    if actor_id <= 0 {
        return -1;
    }
    match registry().write() {
        Ok(mut reg) => {
            if reg.heartbeat(actor_id as u64, bytes_delta) {
                0
            } else {
                -1
            }
        }
        Err(_) => -1,
    }
}

/// Post a message to the unified mailbox. Returns 0 on success, -1 on error.
#[no_mangle]
pub extern "C" fn harmonia_actor_post(
    source: i64,
    target: i64,
    payload_sexp: *const c_char,
) -> i32 {
    let payload_str = match cstr_to_string_local(payload_sexp) {
        Ok(v) => v,
        Err(_) => return -1,
    };
    match registry().write() {
        Ok(mut reg) => {
            let kind = reg
                .actors
                .get(&(source as u64))
                .map(|r| r.kind.clone())
                .unwrap_or(ActorKind::CliAgent);
            let msg = HarmoniaMessage {
                id: reg.mailbox.len() as u64 + 1,
                source: source as u64,
                target: target as u64,
                kind,
                timestamp: harmonia_actor_protocol::now_unix(),
                payload: MessagePayload::StateChanged { to: payload_str },
            };
            reg.post(msg);
            0
        }
        Err(_) => -1,
    }
}

/// Drain ALL messages from the unified mailbox as an s-expression string.
/// Caller must free with harmonia_actor_free_string.
#[no_mangle]
pub extern "C" fn harmonia_actor_drain() -> *mut c_char {
    match registry().write() {
        Ok(mut reg) => to_c_string_local(reg.drain_sexp()),
        Err(_) => to_c_string_local("()".to_string()),
    }
}

/// Get actor state as s-expression. Caller must free with harmonia_actor_free_string.
#[no_mangle]
pub extern "C" fn harmonia_actor_state(actor_id: i64) -> *mut c_char {
    match registry().read() {
        Ok(reg) => to_c_string_local(reg.actor_state_sexp(actor_id as u64)),
        Err(_) => to_c_string_local("(:error \"registry lock poisoned\")".to_string()),
    }
}

/// List all registered actors as s-expression. Caller must free with harmonia_actor_free_string.
#[no_mangle]
pub extern "C" fn harmonia_actor_list() -> *mut c_char {
    match registry().read() {
        Ok(reg) => to_c_string_local(reg.list_sexp()),
        Err(_) => to_c_string_local("()".to_string()),
    }
}

/// Deregister an actor. Returns 0 if found and removed, -1 if not found.
#[no_mangle]
pub extern "C" fn harmonia_actor_deregister(actor_id: i64) -> i32 {
    if actor_id <= 0 {
        return -1;
    }
    match registry().write() {
        Ok(mut reg) => {
            if reg.deregister(actor_id as u64) {
                0
            } else {
                -1
            }
        }
        Err(_) => -1,
    }
}

/// Free a string returned by actor protocol FFI functions.
#[no_mangle]
pub extern "C" fn harmonia_actor_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}
