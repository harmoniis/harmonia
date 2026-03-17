use crate::registry::Registry;
use crate::tool_registry::ToolRegistry;
use parking_lot::RwLock;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::OnceLock;

static GATEWAY: OnceLock<GatewayState> = OnceLock::new();
static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
/// Actor ID assigned by the unified registry (0 = not registered yet)
static ACTOR_ID: AtomicU64 = AtomicU64::new(0);

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

pub struct GatewayState {
    pub registry: Registry,
    pub tool_registry: ToolRegistry,
    pub initialized: RwLock<bool>,
}

pub fn gateway() -> &'static GatewayState {
    GATEWAY.get_or_init(|| GatewayState {
        registry: Registry::new(),
        tool_registry: ToolRegistry::new(),
        initialized: RwLock::new(false),
    })
}

pub fn init() -> Result<(), String> {
    let gw = gateway();
    let mut init = gw.initialized.write();
    if *init {
        return Ok(());
    }
    *init = true;

    // Register as a gateway actor in the unified registry (via dlsym)
    if harmonia_actor_protocol::client::is_available() {
        match harmonia_actor_protocol::client::register("gateway") {
            Ok(id) => {
                ACTOR_ID.store(id, Ordering::SeqCst);
                log::info!("gateway registered as actor {}", id);
            }
            Err(e) => {
                log::warn!("gateway actor registration failed: {}", e);
            }
        }
    }

    Ok(())
}

pub fn actor_id() -> u64 {
    ACTOR_ID.load(Ordering::SeqCst)
}

pub fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub fn set_error(msg: impl Into<String>) {
    *last_error().write() = msg.into();
}

pub fn clear_error() {
    last_error().write().clear();
}

// ── Command query callback ────────────────────────────────────────────────

fn command_query_lock() -> &'static RwLock<Option<CommandQueryFn>> {
    COMMAND_QUERY.get_or_init(|| RwLock::new(None))
}

pub fn set_command_query(handler: Option<CommandQueryFn>) {
    *command_query_lock().write() = handler;
}

pub fn command_query() -> Option<CommandQueryFn> {
    *command_query_lock().read()
}

// ── Payment policy callback ──────────────────────────────────────────────

fn payment_policy_query_lock() -> &'static RwLock<Option<PaymentPolicyQueryFn>> {
    PAYMENT_POLICY_QUERY.get_or_init(|| RwLock::new(None))
}

pub fn set_payment_policy_query(handler: Option<PaymentPolicyQueryFn>) {
    *payment_policy_query_lock().write() = handler;
}

pub fn payment_policy_query() -> Option<PaymentPolicyQueryFn> {
    *payment_policy_query_lock().read()
}

// ── Pending exit flag ─────────────────────────────────────────────────────

pub fn set_pending_exit(value: bool) {
    PENDING_EXIT.store(value, Ordering::SeqCst);
}

pub fn pending_exit() -> bool {
    PENDING_EXIT.load(Ordering::SeqCst)
}
