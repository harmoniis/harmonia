//! Actor-owned Email frontend (IMAP IDLE-style polling + SMTP submission)
//! with PGP sign/verify of message bodies.
//!
//! Replaces the singleton `harmonia-email-client` crate. State lives on
//! the [`EmailFrontend`] struct; the [`Frontend`] trait actor wrapper drives
//! poll/send via blocking I/O wrapped in `tokio::task::spawn_blocking` so
//! the IMAP/SMTP libraries (sync) don't stall the actor task.
//!
//! Wire format:
//! * Inbound: every message body is scanned for an inline clearsigned PGP
//!   block; the metadata stamp records signed/unsigned state. Full
//!   crypto verification of the embedded signature is interlocked with
//!   the trust-store actor's `VerifyClearsigned` variant (later phase).
//! * Outbound: agent-emitted messages are sent plaintext with an
//!   `X-Harmonia-Pgp-Signature` header carrying an ASCII-armored
//!   detached signature over the body, produced by the shared
//!   transport-pgp signer — same crypto path that signs MQTT payloads
//!   and the SIP `X-Harmonia-Sig` header.

mod imap_io;
mod parsing;
mod smtp_io;

pub mod frontend;

pub use frontend::EmailFrontend;
