//! Thread-local trace context for automatic span correlation.
//!
//! When Rust code runs within a traced scope, child spans automatically
//! inherit the parent's trace_id and dotted_order without manual threading.

use std::cell::RefCell;

/// Active trace context for the current thread.
#[derive(Debug, Clone, Default)]
pub struct TraceContext {
    pub trace_id: Option<String>,
    pub parent_run_id: Option<String>,
    pub dotted_order: Option<String>,
}

thread_local! {
    static CURRENT: RefCell<TraceContext> = RefCell::new(TraceContext::default());
}

/// Get the current thread-local trace context.
pub fn current() -> TraceContext {
    CURRENT.with(|c| c.borrow().clone())
}

/// Set the thread-local trace context. Returns the previous context.
pub fn set(ctx: TraceContext) -> TraceContext {
    CURRENT.with(|c| {
        let prev = c.borrow().clone();
        *c.borrow_mut() = ctx;
        prev
    })
}

/// Clear the thread-local trace context.
pub fn clear() {
    CURRENT.with(|c| {
        *c.borrow_mut() = TraceContext::default();
    });
}

/// RAII guard that restores the previous trace context on drop.
pub struct TraceContextGuard {
    previous: TraceContext,
}

impl TraceContextGuard {
    pub fn new(ctx: TraceContext) -> Self {
        let previous = set(ctx);
        Self { previous }
    }
}

impl Drop for TraceContextGuard {
    fn drop(&mut self) {
        set(self.previous.clone());
    }
}
