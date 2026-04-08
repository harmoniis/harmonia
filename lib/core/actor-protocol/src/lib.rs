//! Unified actor protocol types and sexp toolkit for the Harmonia agent.
//!
//! No global state, no statics, no FFI.
//! ComponentDescriptor is the universal trait — one impl = one pluggable component.

pub mod actor;
pub mod component;
pub mod message;
pub mod payload;
pub mod registration;
pub mod sexp;

// Re-export all message types at crate root for backward compat.
pub use actor::{ActorId, ActorKind, ActorState};
pub use message::{HarmoniaMessage, now_unix};
pub use payload::MessagePayload;
pub use registration::ActorRegistration;
pub use component::ComponentDescriptor;

// Re-export sexp toolkit at crate root for backward compat.
pub use sexp::escape as sexp_escape;
pub use sexp::extract_bool as extract_sexp_bool;
pub use sexp::extract_f64 as extract_sexp_f64;
pub use sexp::extract_string as extract_sexp_string;
pub use sexp::extract_string_list as extract_sexp_string_list;
pub use sexp::extract_u64 as extract_sexp_u64;
pub use sexp::extract_u64_or as extract_sexp_u64_or;
