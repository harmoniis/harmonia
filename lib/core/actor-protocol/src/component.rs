//! ComponentDescriptor — the universal trait for pluggable actor components.
//!
//! Every component implements this. The trait IS the protocol.
//! One trait impl = complete component definition. No hardcoding anywhere.
//!
//! ```ignore
//! impl ComponentDescriptor for MyComponent {
//!     const NAME: &'static str = "my-component";
//!     type State = MyState;
//!     fn init() -> Self::State { MyState::new() }
//!     fn dispatch(state: &mut Self::State, sexp: &str) -> String { ... }
//! }
//! ```

/// The universal component protocol. Pure functional: sexp in → sexp out.
/// Actor lifecycle (init, dispatch, tick, shutdown) fully declared.
pub trait ComponentDescriptor: Send + Sync + 'static {
    /// Unique component name (IPC routing key, registry key, log prefix).
    const NAME: &'static str;
    /// Actor-owned state type. No singletons — state lives in the actor.
    type State: Send + 'static;
    /// Create initial state. Called once at actor pre_start.
    fn init() -> Self::State;
    /// Dispatch an IPC command. Pure: sexp in → sexp out. No side effects beyond state.
    fn dispatch(state: &mut Self::State, sexp: &str) -> String;
    /// Periodic tick (heartbeat). Default: no-op.
    fn tick(_state: &mut Self::State) {}
    /// Graceful shutdown. Default: no-op.
    fn shutdown(_state: &mut Self::State) {}
    /// Declared capabilities for pub/sub topic subscription.
    fn capabilities() -> &'static [&'static str] { &[] }
}
