pub mod model;
pub mod runtime;

// Re-export key types for ergonomic access by the runtime actor.
pub use model::{Edge, MatrixEvent, RouteSample, State, StoreConfig, StoreKind};
