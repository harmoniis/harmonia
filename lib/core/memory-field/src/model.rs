/// Core data model for the memory field system.

use std::sync::{Mutex, OnceLock};

/// Global field state — initialized lazily, protected by mutex.
pub(crate) static STATE: OnceLock<Mutex<crate::FieldState>> = OnceLock::new();

/// Global last error — same pattern as signalograd.
pub(crate) static LAST_ERROR: OnceLock<Mutex<String>> = OnceLock::new();
