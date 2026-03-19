use std::sync::atomic::{AtomicBool, Ordering};

/// Set when /exit is intercepted; runtime checks this to stop the run-loop.
static PENDING_EXIT: AtomicBool = AtomicBool::new(false);

// ── Pending exit flag ─────────────────────────────────────────────────────

pub fn set_pending_exit(value: bool) {
    PENDING_EXIT.store(value, Ordering::SeqCst);
}
