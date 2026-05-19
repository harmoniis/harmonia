//! Operator-side TUI renderer for the Harmonia agent.
//!
//! This crate contains *only* the client-side modules — what runs on the
//! operator's terminal when they invoke `harmonia` (with no args). The
//! server-side socket listener lives in
//! [`harmonia_tui_server`](../../tui-server). Splitting them keeps the
//! renderer self-contained (pure ratatui + crossterm + a `SessionHost`
//! trait) so it can also drive a remote tailnet TUI in a later phase
//! without dragging in any in-process server state.

pub mod autocomplete;
pub mod bridge;
pub mod commands;
pub mod input;
pub mod input_loop;
pub mod prompt;
pub mod render;
pub mod session;
pub mod spinner;
pub mod theme;

pub use input::InputCallbacks;
pub use session::{run, SessionHost};
