//! Harmonia Provider Router — multi-backend dispatch with vault-based activation.
//!
//! Routes LLM requests to native provider backends when the user has configured
//! an API key in vault during `harmonia setup`. Falls back to OpenRouter as the
//! universal gateway when no native key is available for the requested model's
//! provider.
//!
//! ## Architecture
//!
//! - `registry` — static provider table, vault key detection, prefix matching
//! - `dispatch` — routing logic: native backend → OpenRouter fallback
//! - `init`     — boot sequence for all active backends
//! - `status`   — backend health reporting (sexp for Lisp introspection)
//! - `ffi`      — C-compatible exports for the runtime IPC bridge

mod dispatch;
mod ffi;
mod init;
mod registry;
mod status;

// Re-export FFI surface used by the runtime and IPC dispatch.
pub use ffi::*;
