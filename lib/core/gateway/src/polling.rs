use crate::envelope::now_ms;
use crate::model::{ChannelBatch, ChannelEnvelope};
use crate::registry::Registry;

/// Poll all registered frontends for inbound signals.
///
/// FFI-based frontend polling has been removed -- frontends are now ractor
/// actors that push envelopes directly. This function processes any envelopes
/// that arrive through the registry (currently none via FFI), applies sender
/// policy, payment interception, and command dispatch.
pub fn poll_baseband(registry: &Registry) -> ChannelBatch {
    // No FFI frontends to poll -- actor-based frontends push envelopes via
    // the runtime IPC system. The batch will be empty unless envelopes are
    // injected through some other path.
    let all_envelopes: Vec<ChannelEnvelope> = Vec::new();

    // Apply sender policy: deny-by-default for messaging frontends
    let all_envelopes: Vec<ChannelEnvelope> = all_envelopes
        .into_iter()
        .filter(|env| crate::sender_policy::is_signal_allowed(env))
        .collect();

    let all_envelopes = crate::payment_auth::intercept_paid_actions(registry, all_envelopes);

    // Intercept gateway commands (/wallet, /identity, etc.) — handle in Rust,
    // send response back to the originating frontend, filter them out so the
    // orchestrator only receives agent-level prompts.
    let all_envelopes = crate::command_dispatch::intercept_commands(registry, all_envelopes);

    ChannelBatch {
        envelopes: all_envelopes,
        poll_timestamp_ms: now_ms(),
    }
}
