//! Default deny-all sender policy for messaging frontends.
//!
//! Messaging channels (email, Slack, Discord, Signal, etc.) default to
//! rejecting all incoming signals except from explicitly allowed senders.
//! TUI and MQTT (device-paired) frontends are exempt.
//!
//! Policy configuration is stored in the config-store under scope
//! `"sender-policy"`, component `"gateway"`:
//!   - `allowlist-<frontend>` → comma-separated sender IDs
//!   - `mode-<frontend>`      → `"deny"` (default) or `"allow-all"`
//!
//! The cache lives inside [`SenderPolicyActor`]. The runtime spawns one
//! actor and threads its `ActorRef` to `GatewayActor` (and any future
//! consumer) so policy decisions are made via message-passing rather than
//! a process-wide singleton.

use crate::model::ChannelEnvelope;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Messaging frontends subject to sender filtering.
/// TUI, MQTT, and Tailscale are exempt (device-paired or local).
const MESSAGING_FRONTENDS: &[&str] = &[
    "email", "slack", "discord", "signal", "whatsapp", "telegram",
];

const POLICY_REFRESH_MS: u64 = 30_000;

/// In-memory policy cache. The actor owns one instance; callers reach it
/// via [`SenderPolicyMsg`].
pub struct SenderPolicyCache {
    allowlists: HashMap<String, HashSet<String>>,
    allow_all: HashSet<String>,
    last_loaded_ms: u64,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn ensure_config_store() {
    if let Ok(root) = std::env::var("HARMONIA_STATE_ROOT") {
        if !root.is_empty() {
            let _ = harmonia_config_store::init();
        }
    }
}

impl SenderPolicyCache {
    /// Create an empty deny-all cache.
    pub fn new() -> Self {
        Self {
            allowlists: HashMap::new(),
            allow_all: HashSet::new(),
            last_loaded_ms: 0,
        }
    }

    /// Load policies from config-store.
    pub fn load(&mut self) {
        ensure_config_store();

        self.allowlists.clear();
        self.allow_all.clear();

        for &frontend in MESSAGING_FRONTENDS {
            let mode_key = format!("mode-{}", frontend);
            if let Ok(Some(mode)) =
                harmonia_config_store::get_config("gateway", "sender-policy", &mode_key)
            {
                if mode == "allow-all" {
                    self.allow_all.insert(frontend.to_string());
                }
            }

            let list_key = format!("allowlist-{}", frontend);
            if let Ok(Some(list)) =
                harmonia_config_store::get_config("gateway", "sender-policy", &list_key)
            {
                let senders: HashSet<String> = list
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !senders.is_empty() {
                    self.allowlists.insert(frontend.to_string(), senders);
                }
            }
        }

        self.last_loaded_ms = now_ms();
    }

    /// Refresh if stale (>30s since last load).
    pub fn refresh_if_stale(&mut self) {
        let now = now_ms();
        if now.saturating_sub(self.last_loaded_ms) > POLICY_REFRESH_MS {
            self.load();
        }
    }

    /// Check whether an inbound envelope should be accepted.
    pub fn is_signal_allowed(&self, envelope: &ChannelEnvelope) -> bool {
        let frontend = &envelope.channel.kind;

        // 1. Non-messaging frontends pass through (TUI, MQTT, Tailscale)
        if !MESSAGING_FRONTENDS.contains(&frontend.as_str()) {
            return true;
        }

        // 2. Self-originated signals pass through
        if let Some(ref origin) = envelope.origin {
            if !origin.remote {
                return true;
            }
        }

        // 3. Check allow-all for this frontend
        if self.allow_all.contains(frontend.as_str()) {
            return true;
        }

        // 4. Check if sender is in allowlist
        if let Some(allowed) = self.allowlists.get(frontend.as_str()) {
            let peer_id = envelope.peer.id.to_lowercase();
            if allowed.contains(&peer_id) {
                return true;
            }

            let address = envelope.channel.address.to_lowercase();
            if allowed.contains(&address) {
                return true;
            }
        }

        // Default: deny
        false
    }
}

impl Default for SenderPolicyCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Messages accepted by [`SenderPolicyActor`].
pub enum SenderPolicyMsg {
    /// Apply policy to one envelope. The reply is `true` when the signal
    /// should be accepted, `false` when it must be dropped.
    IsSignalAllowed(ChannelEnvelope, RpcReplyPort<bool>),
    /// Force-reload policies from config-store (admin operations).
    Reload(RpcReplyPort<()>),
}

pub struct SenderPolicyActor;

impl Actor for SenderPolicyActor {
    type Msg = SenderPolicyMsg;
    type State = SenderPolicyCache;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let mut cache = SenderPolicyCache::new();
        cache.load();
        eprintln!("[INFO] [gateway] SenderPolicyActor started");
        Ok(cache)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        cache: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            SenderPolicyMsg::IsSignalAllowed(envelope, reply) => {
                cache.refresh_if_stale();
                let allowed = cache.is_signal_allowed(&envelope);
                let _ = reply.send(allowed);
            }
            SenderPolicyMsg::Reload(reply) => {
                cache.load();
                let _ = reply.send(());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ractor::Actor;

    #[tokio::test]
    async fn actor_handles_reload_round_trip() {
        let (actor, handle) = Actor::spawn(None, SenderPolicyActor, ())
            .await
            .expect("spawn SenderPolicyActor");
        let _: () = ractor::call_t!(actor, SenderPolicyMsg::Reload, 500)
            .expect("Reload should round-trip through the actor");
        actor.stop(None);
        handle.await.expect("actor exits cleanly");
    }
}
