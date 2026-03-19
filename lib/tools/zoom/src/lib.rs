//! Zoom service tool: operates Zoom meetings via browser automation.
//!
//! Built on top of `harmonia-browser` with Chrome CDP and stealth engine.
//! All Zoom interaction happens through the Zoom web client — no SDK or
//! API keys required, just browser-level automation with human-like behavior.
//!
//! ## Operations
//!
//! - `join`             — Join a Zoom meeting via web client
//! - `leave`            — Leave the current meeting
//! - `get-transcript`   — Extract live transcript from meeting
//! - `send-chat`        — Send a chat message in meeting
//! - `get-participants` — List current participants
//! - `get-status`       — Check current meeting status
//!
//! ## Architecture
//!
//! Each operation launches a stealth Chrome instance, navigates to the
//! Zoom web client, and performs DOM interactions using human-like timing.
//! The browser's stealth engine ensures Zoom cannot detect automation.

pub mod operations;

pub use operations::*;
