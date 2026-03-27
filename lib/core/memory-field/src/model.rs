/// Core data model for the memory field system.
///
/// Legacy globals kept for backward compat (api.rs C FFI).
/// The runtime's MemoryFieldActor owns FieldState directly.

use std::sync::{Mutex, OnceLock};

/// Legacy global state — deprecated. Use actor-owned FieldState instead.
pub(crate) static LEGACY_STATE: OnceLock<Mutex<crate::FieldState>> = OnceLock::new();

/// Legacy last error — deprecated. Return Result<T, String> instead.
pub(crate) static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();
