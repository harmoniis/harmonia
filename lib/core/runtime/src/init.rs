//! Component initialization — config-driven, conditional loading.
//!
//! Only components listed in the runtime config are initialized.
//! Core infrastructure (config-store, chronicle, vault) always loads.
//! Everything else loads only when configured and enabled.
//!
//! Config source: config-store key "runtime/components" or env
//! HARMONIA_RUNTIME_COMPONENTS (comma-separated list).
//!
//! Example: HARMONIA_RUNTIME_COMPONENTS=tui,provider-router,voice-router,signalograd

use std::collections::HashSet;
use std::ffi::CString;

/// A loaded component with its name and status.
#[allow(dead_code)]
pub struct ComponentStatus {
    pub name: String,
    pub enabled: bool,
    pub initialized: bool,
    pub error: Option<String>,
}

/// Result of initialization.
pub struct InitResult {
    pub components: Vec<ComponentStatus>,
}

impl InitResult {
    pub fn ok_count(&self) -> usize {
        self.components.iter().filter(|c| c.initialized).count()
    }
    pub fn total_count(&self) -> usize {
        self.components.len()
    }
}

/// Resolve which components should be loaded.
/// Core components always load. Optional components load only if listed.
fn resolve_enabled_components() -> HashSet<String> {
    let mut enabled = HashSet::new();

    // Core — always loaded
    for name in &["config-store", "chronicle", "vault", "memory"] {
        enabled.insert(name.to_string());
    }

    // Check env var first
    if let Ok(raw) = std::env::var("HARMONIA_RUNTIME_COMPONENTS") {
        for name in raw.split(',') {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                enabled.insert(trimmed.to_string());
            }
        }
        return enabled;
    }

    // Check config-store
    if let Ok(Some(raw)) =
        harmonia_config_store::get_config("harmonia-runtime", "runtime", "components")
    {
        for name in raw.split(',') {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                enabled.insert(trimmed.to_string());
            }
        }
        return enabled;
    }

    // Default: load everything for backward compatibility
    for name in &[
        "tui",
        "signalograd",
        "harmonic-matrix",
        "provider-router",
        "voice-router",
        "tailnet",
        "telegram",
        "slack",
        "discord",
        "signal",
        "mattermost",
        "nostr",
        "email",
        "whatsapp",
        "imessage",
        "tailscale",
    ] {
        enabled.insert(name.to_string());
    }

    enabled
}

/// Initialize all enabled components.
pub fn init_all() -> InitResult {
    let enabled = resolve_enabled_components();
    let mut result = InitResult {
        components: Vec::new(),
    };

    eprintln!("[INFO] [init] Components enabled: {}", {
        let mut names: Vec<&String> = enabled.iter().collect();
        names.sort();
        names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    });

    // ── Core (always) ────────────────────────────────────────────────

    init_component(&mut result, "config-store", true, || {
        harmonia_config_store::init().map_err(|e| e.to_string())
    });

    init_component(&mut result, "chronicle", true, || {
        harmonia_chronicle::init().map_err(|e| e.to_string())
    });

    init_component(&mut result, "vault", true, || {
        harmonia_vault::init_from_env().map_err(|e| e.to_string())
    });

    init_component(&mut result, "memory", true, || {
        let path = memory_db_path();
        let c = CString::new(path.as_str()).unwrap_or_default();
        let rc = harmonia_memory::harmonia_memory_init(c.as_ptr());
        if rc == 0 {
            Ok(())
        } else {
            Err(format!("memory init returned {rc}"))
        }
    });

    // ── Optional components (config-driven) ──────────────────────────

    if enabled.contains("signalograd") {
        init_component(&mut result, "signalograd", true, || {
            let rc = harmonia_signalograd::harmonia_signalograd_init();
            if rc == 0 {
                Ok(())
            } else {
                Err("signalograd init failed".into())
            }
        });
    }

    if enabled.contains("harmonic-matrix") {
        init_component(&mut result, "harmonic-matrix", true, || {
            harmonia_harmonic_matrix::runtime::store::init().map_err(|e| e.to_string())
        });
    }

    if enabled.contains("tui") {
        init_component(&mut result, "tui", true, || {
            harmonia_tui::terminal::init().map_err(|e| e)
        });
    }

    // ── Frontends (only if configured) ───────────────────────────────

    if enabled.contains("telegram") {
        init_component(&mut result, "telegram", false, || {
            harmonia_telegram::bot::init("()")
        });
    }

    if enabled.contains("slack") {
        init_component(&mut result, "slack", false, || {
            harmonia_slack::client::init("()")
        });
    }

    if enabled.contains("discord") {
        init_component(&mut result, "discord", false, || {
            harmonia_discord::client::init("()")
        });
    }

    if enabled.contains("signal") {
        init_component(&mut result, "signal", false, || {
            harmonia_signal::client::init("()")
        });
    }

    if enabled.contains("mattermost") {
        init_component(&mut result, "mattermost", false, || {
            harmonia_mattermost::client::init("()")
        });
    }

    if enabled.contains("nostr") {
        init_component(&mut result, "nostr", false, || {
            harmonia_nostr::client::init("()")
        });
    }

    if enabled.contains("email") {
        init_component(&mut result, "email", false, || {
            harmonia_email_client::client::init("()")
        });
    }

    if enabled.contains("whatsapp") {
        init_component(&mut result, "whatsapp", false, || {
            harmonia_whatsapp::client::init("()")
        });
    }

    #[cfg(target_os = "macos")]
    if enabled.contains("imessage") {
        init_component(&mut result, "imessage", false, || {
            harmonia_imessage::client::init("()")
        });
    }

    if enabled.contains("tailscale") {
        init_component(&mut result, "tailscale", false, || {
            harmonia_tailscale_frontend::bridge::init("()")
        });
    }

    // ── Backends ─────────────────────────────────────────────────────

    if enabled.contains("provider-router") {
        init_component(&mut result, "provider-router", true, || {
            let rc = harmonia_provider_router::harmonia_provider_router_init();
            if rc == 0 {
                Ok(())
            } else {
                Err("provider-router init failed".into())
            }
        });
    }

    if enabled.contains("voice-router") {
        init_component(&mut result, "voice-router", false, || {
            harmonia_voice_router::init().map_err(|e| e.to_string())
        });
    }

    // ── Tailnet ──────────────────────────────────────────────────────

    if enabled.contains("tailnet") {
        init_component(&mut result, "tailnet", false, || {
            harmonia_tailnet::transport::start_listener().map_err(|e| e)
        });
    }

    // ── Summary ──────────────────────────────────────────────────────

    let ok = result.ok_count();
    let total = result.total_count();
    eprintln!("[INFO] [init] Initialization complete: {ok}/{total} components ready");

    result
}

fn init_component(
    result: &mut InitResult,
    name: &str,
    core: bool,
    f: impl FnOnce() -> Result<(), String>,
) {
    let status = match f() {
        Ok(()) => {
            eprintln!("[INFO] [init] {name} initialized");
            ComponentStatus {
                name: name.to_string(),
                enabled: true,
                initialized: true,
                error: None,
            }
        }
        Err(e) => {
            if core {
                eprintln!("[WARN] [init] {name} failed: {e}");
            } else {
                eprintln!("[INFO] [init] {name}: {e}");
            }
            ComponentStatus {
                name: name.to_string(),
                enabled: true,
                initialized: false,
                error: Some(e),
            }
        }
    };
    result.components.push(status);
}

fn memory_db_path() -> String {
    harmonia_config_store::get_config_or(
        "harmonia-runtime",
        "global",
        "state-root",
        "/tmp/harmonia",
    )
    .map(|root| format!("{}/memory.db", root))
    .unwrap_or_else(|_| "/tmp/harmonia/memory.db".to_string())
}
