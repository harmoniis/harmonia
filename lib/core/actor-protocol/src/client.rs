//! Runtime FFI client for the unified actor registry.
//!
//! The ONE actor registry lives in `libharmonia_parallel_agents.dylib`.
//! This module uses `dlsym(RTLD_DEFAULT, ...)` to find the FFI functions
//! at runtime, so any crate (gateway, tailnet, chronicle) loaded into the
//! same process can call them without a static link dependency on the
//! parallel-agents crate — which would create duplicate registries.
//!
//! All functions are safe Rust wrappers that return `Result<_, String>`.

use std::ffi::{c_void, CString};
use std::os::raw::c_char;
use std::sync::OnceLock;

// ─── Platform-specific RTLD_DEFAULT ─────────────────────────────────────

#[cfg(target_os = "macos")]
const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;

#[cfg(target_os = "linux")]
const RTLD_DEFAULT: *mut c_void = std::ptr::null_mut();

#[cfg(any(target_os = "macos", target_os = "linux"))]
extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleA(module_name: *const c_char) -> *mut c_void;
    fn GetProcAddress(module: *mut c_void, proc_name: *const c_char) -> *mut c_void;
}

// ─── Function pointer types ─────────────────────────────────────────────

type RegisterFn = unsafe extern "C" fn(*const c_char) -> i64;
type HeartbeatFn = unsafe extern "C" fn(i64, u64) -> i32;
type PostFn = unsafe extern "C" fn(i64, i64, *const c_char) -> i32;
type DrainFn = unsafe extern "C" fn() -> *mut c_char;
type StateFn = unsafe extern "C" fn(i64) -> *mut c_char;
type ListFn = unsafe extern "C" fn() -> *mut c_char;
type DeregisterFn = unsafe extern "C" fn(i64) -> i32;
type FreeStringFn = unsafe extern "C" fn(*mut c_char);

// ─── Lazy resolution ────────────────────────────────────────────────────

#[cfg(any(target_os = "macos", target_os = "linux"))]
fn resolve_sym(name: &[u8]) -> *mut c_void {
    // name must be null-terminated
    unsafe { dlsym(RTLD_DEFAULT, name.as_ptr().cast()) }
}

#[cfg(windows)]
fn resolve_sym(name: &[u8]) -> *mut c_void {
    // On Windows, the registry symbols live in the already-loaded
    // parallel-agents DLL. Probe the common cargo/CFFI module names.
    const MODULES: [&[u8]; 2] = [
        b"harmonia_parallel_agents.dll\0",
        b"libharmonia_parallel_agents.dll\0",
    ];

    for module in MODULES {
        let handle = unsafe { GetModuleHandleA(module.as_ptr().cast()) };
        if !handle.is_null() {
            let ptr = unsafe { GetProcAddress(handle, name.as_ptr().cast()) };
            if !ptr.is_null() {
                return ptr;
            }
        }
    }

    std::ptr::null_mut()
}

macro_rules! resolve_fn {
    ($cache:ident, $getter:ident, $sym:expr, $ty:ty) => {
        static $cache: OnceLock<Option<$ty>> = OnceLock::new();
        fn $getter() -> Option<$ty> {
            *$cache.get_or_init(|| {
                let ptr = resolve_sym($sym);
                if ptr.is_null() {
                    None
                } else {
                    Some(unsafe { std::mem::transmute(ptr) })
                }
            })
        }
    };
}

resolve_fn!(
    CACHE_REGISTER,
    get_register_fn,
    b"harmonia_actor_register\0",
    RegisterFn
);
resolve_fn!(
    CACHE_HEARTBEAT,
    get_heartbeat_fn,
    b"harmonia_actor_heartbeat\0",
    HeartbeatFn
);
resolve_fn!(CACHE_POST, get_post_fn, b"harmonia_actor_post\0", PostFn);
resolve_fn!(
    CACHE_DRAIN,
    get_drain_fn,
    b"harmonia_actor_drain\0",
    DrainFn
);
resolve_fn!(
    CACHE_STATE,
    get_state_fn,
    b"harmonia_actor_state\0",
    StateFn
);
resolve_fn!(CACHE_LIST, get_list_fn, b"harmonia_actor_list\0", ListFn);
resolve_fn!(
    CACHE_DEREGISTER,
    get_deregister_fn,
    b"harmonia_actor_deregister\0",
    DeregisterFn
);
resolve_fn!(
    CACHE_FREE_STRING,
    get_free_string_fn,
    b"harmonia_actor_free_string\0",
    FreeStringFn
);

const NOT_LOADED: &str = "actor registry not available (parallel-agents not loaded)";

// ─── Safe wrappers ──────────────────────────────────────────────────────

/// Register an actor of the given kind string. Returns actor ID.
pub fn register(kind: &str) -> Result<u64, String> {
    let f = get_register_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let ckind = CString::new(kind).map_err(|e| e.to_string())?;
    let id = unsafe { f(ckind.as_ptr()) };
    if id < 0 {
        Err("actor registration failed".to_string())
    } else {
        Ok(id as u64)
    }
}

/// Send a heartbeat for the given actor.
pub fn heartbeat(actor_id: u64, bytes_delta: u64) -> Result<(), String> {
    let f = get_heartbeat_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let rc = unsafe { f(actor_id as i64, bytes_delta) };
    if rc == 0 {
        Ok(())
    } else {
        Err("heartbeat failed (unknown actor?)".to_string())
    }
}

/// Post a message to the unified mailbox (generic sexp payload).
pub fn post(source: u64, target: u64, payload_sexp: &str) -> Result<(), String> {
    let f = get_post_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let cpayload = CString::new(payload_sexp).map_err(|e| e.to_string())?;
    let rc = unsafe { f(source as i64, target as i64, cpayload.as_ptr()) };
    if rc == 0 {
        Ok(())
    } else {
        Err("post failed".to_string())
    }
}

/// Drain all messages from the unified mailbox. Returns s-expression string.
pub fn drain() -> Result<String, String> {
    let f = get_drain_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let ptr = unsafe { f() };
    ptr_to_string(ptr)
}

/// Get actor state as s-expression.
pub fn actor_state(actor_id: u64) -> Result<String, String> {
    let f = get_state_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let ptr = unsafe { f(actor_id as i64) };
    ptr_to_string(ptr)
}

/// List all registered actors as s-expression.
pub fn list() -> Result<String, String> {
    let f = get_list_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let ptr = unsafe { f() };
    ptr_to_string(ptr)
}

/// Deregister an actor.
pub fn deregister(actor_id: u64) -> Result<(), String> {
    let f = get_deregister_fn().ok_or_else(|| NOT_LOADED.to_string())?;
    let rc = unsafe { f(actor_id as i64) };
    if rc == 0 {
        Ok(())
    } else {
        Err("deregister failed (unknown actor?)".to_string())
    }
}

/// Check if the actor registry is available (parallel-agents loaded).
pub fn is_available() -> bool {
    get_register_fn().is_some()
}

// ─── Internal helpers ───────────────────────────────────────────────────

fn ptr_to_string(ptr: *mut c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Ok("()".to_string());
    }
    let s = unsafe { std::ffi::CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    // Free via the registry's free function
    if let Some(free_fn) = get_free_string_fn() {
        unsafe { free_fn(ptr) };
    }
    Ok(s)
}
