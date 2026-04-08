// Baseband: re-exports from focused sub-modules.
//
// - envelope: envelope building, dissonance computation, metadata parsing
// - polling:  poll_baseband, parse_frontend_envelopes
// - sending:  send_signal stub

pub use crate::polling::poll_baseband;
pub use crate::sending::send_signal;
