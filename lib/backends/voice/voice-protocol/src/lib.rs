//! Harmonia Voice Provider Protocol
//!
//! Standardised types, HTTP helpers, offering management, performance logging,
//! and FFI scaffolding shared by every voice backend crate (whisper, elevenlabs, etc.).

mod http;
mod offering;
mod state;

pub use http::*;
pub use offering::*;
pub use state::*;

pub use harmonia_vault;
