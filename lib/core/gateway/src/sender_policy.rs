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

use crate::model::ChannelEnvelope;
use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

/// Messaging frontends subject to sender filtering.
/// TUI, MQTT, and Tailscale are exempt (device-paired or local).
const MESSAGING_FRONTENDS: &[&str] = &[
    "email", "slack", "discord", "mattermost", "signal",
    "whatsapp", "imessage", "telegram", "nostr",
];

const POLICY_REFRESH_MS: u64 = 30_000;

/// Actor-owned sender policy cache. No global singleton.
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

// ── Module-level convenience functions (delegate to a process-level instance) ──
// GatewayActor owns its own SenderPolicyCache. These process-level functions
// exist for callers outside the actor (e.g., baseband poll filtering in dispatch).

use std::sync::{Mutex, OnceLock};

static PROCESS_CACHE: OnceLock<Mutex<SenderPolicyCache>> = OnceLock::new();

fn process_cache() -> &'static Mutex<SenderPolicyCache> {
    PROCESS_CACHE.get_or_init(|| {
        let mut cache = SenderPolicyCache::new();
        cache.load();
        Mutex::new(cache)
    })
}

/// Check whether an inbound envelope should be accepted (process-level).
pub fn is_signal_allowed(envelope: &ChannelEnvelope) -> bool {
    let mut guard = process_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.refresh_if_stale();
    guard.is_signal_allowed(envelope)
}

/// Force-reload policies from config-store.
pub fn reload_policies() {
    let mut guard = process_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.load();
}
