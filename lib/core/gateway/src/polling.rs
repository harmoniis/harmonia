use crate::envelope::now_ms;
use crate::model::{ChannelBatch, ChannelEnvelope};
use crate::registry::Registry;

/// Poll all registered frontends for inbound signals.
///
/// FFI-based frontend polling has been removed -- frontends are now ractor
/// actors that push envelopes directly. This function processes any envelopes
/// that arrive through the registry (currently none via FFI), applies
/// payment interception, and command dispatch. Sender-policy filtering runs
/// in the caller (GatewayActor) against [`crate::SenderPolicyActor`], so the
/// gateway library stays sync-pure and the policy cache stays actor-owned.
pub fn poll_baseband(registry: &Registry) -> ChannelBatch {
    // No FFI frontends to poll -- actor-based frontends push envelopes via
    // the runtime IPC system. The batch will be empty unless envelopes are
    // injected through some other path.
    let all_envelopes: Vec<ChannelEnvelope> = Vec::new();

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
