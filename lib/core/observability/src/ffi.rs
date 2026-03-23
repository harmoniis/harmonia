//! Public API for observability — called by IPC dispatch and actors.
//!
//! Design principles:
//! - Never panic, never block the caller
//! - No-op when disabled — zero cost
//! - Config loaded once into OnceLock, actor ref stored for trace dispatch
//! - Level checks read config directly (no actor query, no mutex)
//! - Provider selection is config-driven, handled by providers::from_config

use std::sync::mpsc::SyncSender;
use std::sync::OnceLock;

use ractor::ActorRef;

use crate::config::ObservabilityConfig;
use crate::model::{ObsMsg, TraceLevel, TraceMessage};
use crate::sender;

static CONFIG: OnceLock<ObservabilityConfig> = OnceLock::new();
static OBS_ACTOR: OnceLock<ActorRef<ObsMsg>> = OnceLock::new();
static SENDER_HANDLE: OnceLock<SyncSender<TraceMessage>> = OnceLock::new();

// ─── Lifecycle ───────────────────────────────────────────────────────

/// Initialize observability. Loads config into OnceLock. Returns 0 on success. Idempotent.
pub fn harmonia_observability_init() -> i32 {
    if CONFIG.get().is_some() {
        return 0;
    }

    let config = ObservabilityConfig::load();

    if !config.enabled || config.backend.is_empty() {
        eprintln!(
            "[INFO] [observability] Disabled (enabled={}, backend={})",
            config.enabled,
            if config.backend.is_empty() { "<none>" } else { &config.backend }
        );
    } else {
        eprintln!(
            "[INFO] [observability] Initialized (provider={}, level={}, sample_rate={})",
            config.backend,
            config.trace_level.as_str(),
            config.sample_rate
        );
    }

    let _ = CONFIG.set(config);
    0
}

/// Start the sender thread and return a clone of the sender handle.
/// Resolves the provider from config. Returns None if no provider is
/// configured or the provider fails validation.
pub fn start_sender() -> Option<SyncSender<TraceMessage>> {
    let config = CONFIG.get()?;
    if !config.enabled || config.backend.is_empty() {
        return None;
    }
    let sender = SENDER_HANDLE.get_or_init(|| {
        let provider = crate::providers::from_config(config).unwrap_or_else(|| {
            eprintln!("[WARN] [observability] Provider '{}' failed to initialize, using noop", config.backend);
            Box::new(NoopBackend)
        });
        eprintln!("[INFO] [observability] Provider: {}", provider.name());
        sender::start_with_backend(provider)
    });
    Some(sender.clone())
}

/// Store the ObservabilityActor ref for IPC dispatch bridge and trace helpers.
pub fn set_obs_actor(actor: ActorRef<ObsMsg>) {
    let _ = OBS_ACTOR.set(actor);
}

/// Get the stored ObservabilityActor ref.
pub fn get_obs_actor() -> Option<&'static ActorRef<ObsMsg>> {
    OBS_ACTOR.get()
}

/// Get the stored config.
pub fn get_config() -> Option<&'static ObservabilityConfig> {
    CONFIG.get()
}

pub fn harmonia_observability_flush() {
    if let Some(obs) = OBS_ACTOR.get() {
        let _ = obs.cast(ObsMsg::Flush);
    }
}

pub fn harmonia_observability_shutdown() {
    if let Some(obs) = OBS_ACTOR.get() {
        let _ = obs.cast(ObsMsg::Shutdown);
    }
}

// ─── Level checks ───────────────────────────────────────────────────

pub fn harmonia_observability_enabled() -> bool {
    CONFIG
        .get()
        .map(|c| c.enabled && !c.backend.is_empty())
        .unwrap_or(false)
}

pub fn harmonia_observability_is_standard() -> bool {
    CONFIG
        .get()
        .map(|c| {
            c.enabled
                && !c.backend.is_empty()
                && matches!(c.trace_level, TraceLevel::Standard | TraceLevel::Verbose)
        })
        .unwrap_or(false)
}

pub fn harmonia_observability_is_verbose() -> bool {
    CONFIG
        .get()
        .map(|c| c.enabled && !c.backend.is_empty() && c.trace_level == TraceLevel::Verbose)
        .unwrap_or(false)
}

// ─── Noop fallback (provider init failed but sender still needs something) ──

struct NoopBackend;

impl crate::backend::TraceBackend for NoopBackend {
    fn submit_batch(&self, _creates: &[serde_json::Value], _updates: &[serde_json::Value]) -> crate::backend::FlushResult {
        crate::backend::FlushResult::Ok
    }
    fn name(&self) -> &'static str { "noop" }
}
