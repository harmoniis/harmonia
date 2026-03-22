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

use std::collections::{HashMap, HashSet};

use crate::registry::{self, ModuleEntry, ModuleStatus};

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
        "observability",
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

/// Initialize all enabled components using the module registry.
///
/// Returns a HashMap of module name → ModuleEntry, suitable for
/// embedding in the supervisor state for runtime load/unload.
pub fn init_all() -> HashMap<String, ModuleEntry> {
    let enabled = resolve_enabled_components();
    let modules = registry::build_registry();

    eprintln!("[INFO] [init] Components enabled: {}", {
        let mut names: Vec<&String> = enabled.iter().collect();
        names.sort();
        names
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    });

    let mut registry_map: HashMap<String, ModuleEntry> = HashMap::new();
    let mut ok_count = 0usize;
    let mut total_count = 0usize;

    for mut entry in modules {
        let name = entry.name.clone();
        let should_init = entry.core || enabled.contains(&name);

        if !should_init {
            // Module exists in registry but is not enabled — keep as Unloaded
            registry_map.insert(name, entry);
            continue;
        }

        total_count += 1;

        // Validate config requirements first
        if let Err(e) = registry::validate_config(&entry.config_reqs) {
            if entry.core {
                eprintln!("[WARN] [init] {name} config check failed: {e}");
            } else {
                eprintln!("[INFO] [init] {name}: {e}");
            }
            entry.status = ModuleStatus::Error(e);
            registry_map.insert(name, entry);
            continue;
        }

        // Try to initialize
        match (entry.init_fn)() {
            Ok(()) => {
                eprintln!("[INFO] [init] {name} initialized");
                entry.status = ModuleStatus::Loaded;
                ok_count += 1;
            }
            Err(e) => {
                if entry.core {
                    eprintln!("[WARN] [init] {name} failed: {e}");
                } else {
                    eprintln!("[INFO] [init] {name}: {e}");
                }
                entry.status = ModuleStatus::Error(e);
            }
        }

        registry_map.insert(name, entry);
    }

    eprintln!("[INFO] [init] Initialization complete: {ok_count}/{total_count} components ready");

    registry_map
}

pub(crate) fn memory_db_path() -> String {
    harmonia_config_store::get_config_or(
        "harmonia-runtime",
        "global",
        "state-root",
        "/tmp/harmonia",
    )
    .map(|root| format!("{}/memory.db", root))
    .unwrap_or_else(|_| "/tmp/harmonia/memory.db".to_string())
}
