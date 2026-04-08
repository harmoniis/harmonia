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
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Messaging frontends subject to sender filtering.
/// TUI, MQTT, and Tailscale are exempt (device-paired or local).
const MESSAGING_FRONTENDS: &[&str] = &[
    "email",
    "slack",
    "discord",
    "mattermost",
    "signal",
    "whatsapp",
    "imessage",
    "telegram",
    "nostr",
];

const POLICY_REFRESH_MS: u64 = 30_000;

struct SenderPolicyCache {
    allowlists: HashMap<String, HashSet<String>>,
    allow_all: HashSet<String>,
    last_loaded_ms: u64,
}

static CACHE: Mutex<Option<SenderPolicyCache>> = Mutex::new(None);

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

fn load_policies() -> SenderPolicyCache {
    ensure_config_store();

    let mut allowlists = HashMap::new();
    let mut allow_all = HashSet::new();

    for &frontend in MESSAGING_FRONTENDS {
        // Check mode
        let mode_key = format!("mode-{}", frontend);
        if let Ok(Some(mode)) =
            harmonia_config_store::get_config("gateway", "sender-policy", &mode_key)
        {
            if mode == "allow-all" {
                allow_all.insert(frontend.to_string());
            }
        }

        // Load allowlist
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
                allowlists.insert(frontend.to_string(), senders);
            }
        }
    }

    SenderPolicyCache {
        allowlists,
        allow_all,
        last_loaded_ms: now_ms(),
    }
}

fn with_cache<F, R>(f: F) -> R
where
    F: FnOnce(&SenderPolicyCache) -> R,
{
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    let now = now_ms();
    let needs_refresh = match guard.as_ref() {
        Some(cache) => now.saturating_sub(cache.last_loaded_ms) > POLICY_REFRESH_MS,
        None => true,
    };
    if needs_refresh {
        *guard = Some(load_policies());
    }
    f(guard.as_ref().unwrap())
}

/// Check whether an inbound envelope should be accepted.
///
/// Returns `true` if the signal is allowed, `false` if it should be dropped.
pub fn is_signal_allowed(envelope: &ChannelEnvelope) -> bool {
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

    // 3. Check policy cache
    with_cache(|cache| {
        // If allow-all is set for this frontend, pass through
        if cache.allow_all.contains(frontend.as_str()) {
            return true;
        }

        // Check if sender is in allowlist
        if let Some(allowed) = cache.allowlists.get(frontend.as_str()) {
            let peer_id = envelope.peer.id.to_lowercase();
            if allowed.contains(&peer_id) {
                return true;
            }

            // Also check channel address (e.g., email address, phone number)
            let address = envelope.channel.address.to_lowercase();
            if allowed.contains(&address) {
                return true;
            }
        }

        // Default: deny
        false
    })
}

/// Force-reload policies from config-store (called after TUI updates policies).
pub fn reload_policies() {
    let mut guard = CACHE.lock().unwrap_or_else(|e| e.into_inner());
    *guard = Some(load_policies());
}
