//! HTTP/3 (QUIC + h3) mTLS frontend for the Harmonia agent.
//!
//! Replaces the previous `harmonia-http2-mtls` crate, which exported a C ABI
//! and was loaded as a dynamic library. This crate is a pure-Rust
//! [`harmonia_frontend_trait::Frontend`] actor — the runtime spawns it the
//! same way as Slack/Telegram/MQTT/TUI, with all state owned by the actor
//! and no `OnceLock`/`RwLock` singletons.
//!
//! ## Wire format
//!
//! Same NDJSON-over-stream protocol as before, so existing operator clients
//! keep working:
//!
//! * Operator opens a bidi QUIC stream with HTTP request:
//!   `POST /v1/stream/{session_id}/{channel}`
//! * Each stream-direction frame is one JSON object per line —
//!   `{"payload": "...", "metadata": "..."}` from the client,
//!   `{"payload": "..."}` from the agent's responses.
//! * mTLS via rustls 0.23 — server cert from Vault PKI, client cert
//!   fingerprint must be in the agent's `trusted-client-fingerprints` set.
//!
//! Phase 5b will add a PGP-Hello stream gate (first frame carries a
//! pubkey + signature that the trust-store actor verifies before any
//! payload bytes are forwarded).

mod frontend;
mod model;
mod server;
mod tls;

pub use frontend::Http3Frontend;
