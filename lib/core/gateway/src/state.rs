use parking_lot::RwLock;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;

/// Lisp-provided callback for delegated system commands.
/// Signature: fn(command: *const c_char, args: *const c_char) -> *mut c_char
/// Returns a malloc'd string (caller frees) or null.
pub type CommandQueryFn = unsafe extern "C" fn(*const c_char, *const c_char) -> *mut c_char;
static COMMAND_QUERY: OnceLock<RwLock<Option<CommandQueryFn>>> = OnceLock::new();

/// Lisp-provided callback for payment policy decisions.
/// Signature: fn(summary: *const c_char) -> *mut c_char
/// Returns a malloc'd s-expression string (caller frees) or null.
pub type PaymentPolicyQueryFn = unsafe extern "C" fn(*const c_char) -> *mut c_char;
static PAYMENT_POLICY_QUERY: OnceLock<RwLock<Option<PaymentPolicyQueryFn>>> = OnceLock::new();

/// Set when /exit is intercepted; Lisp checks this to stop the run-loop.
static PENDING_EXIT: AtomicBool = AtomicBool::new(false);

// ── Command query callback ────────────────────────────────────────────────

fn command_query_lock() -> &'static RwLock<Option<CommandQueryFn>> {
    COMMAND_QUERY.get_or_init(|| RwLock::new(None))
}

pub fn command_query() -> Option<CommandQueryFn> {
    *command_query_lock().read()
}

// ── Payment policy callback ──────────────────────────────────────────────

fn payment_policy_query_lock() -> &'static RwLock<Option<PaymentPolicyQueryFn>> {
    PAYMENT_POLICY_QUERY.get_or_init(|| RwLock::new(None))
}

pub fn payment_policy_query() -> Option<PaymentPolicyQueryFn> {
    *payment_policy_query_lock().read()
}

// ── Pending exit flag ─────────────────────────────────────────────────────

pub fn set_pending_exit(value: bool) {
    PENDING_EXIT.store(value, Ordering::SeqCst);
}
