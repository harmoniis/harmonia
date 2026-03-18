//! harmonia-browser: Production-grade secure headless browser tool.
//!
//! Architecture: 3-Layer Security
//!
//! - **Layer 1: Process Isolation** — Browser operations run in sandboxed threads
//!   with strict timeout enforcement. Chrome/Chromium process launch with minimal
//!   privileges available via feature flag.
//!
//! - **Layer 2: HTTP Engine** — Uses `ureq` for HTTP fetching with configurable
//!   timeouts, response size limits, and domain allowlists. Chrome CDP integration
//!   available via `chrome` feature flag. Controlled fetch module blocks dangerous
//!   targets (localhost, metadata endpoints, internal IPs).
//!
//! - **Layer 3: Security Boundary** — Every response is wrapped in a security
//!   boundary that demarcates website data from agent instructions, preventing
//!   prompt injection attacks.
//!
//! ## FFI Design
//!
//! This is a `cdylib` + `rlib` crate loaded into SBCL via CFFI. All async
//! operations are managed internally with a dedicated tokio runtime. FFI
//! functions are synchronous C-ABI exports that block on the internal runtime.
//!
//! ## MCP Surface
//!
//! Two tools are exposed:
//! - `browser_search`: fetch a URL and extract data with a named macro
//! - `browser_execute`: multi-step browser plan (multiple fetch+extract)

pub mod chrome;
pub mod controlled_fetch;
pub mod engine;
pub mod ffi;
pub mod macros;
pub mod mcp;
pub mod sandbox;
pub mod security;
pub mod session;
pub mod stealth;

// Re-export FFI functions at crate root for cdylib symbol visibility.
pub use ffi::*;
