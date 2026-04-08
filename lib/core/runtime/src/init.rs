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

    let registry_map: HashMap<String, ModuleEntry> = modules
        .into_iter()
        .map(|mut entry| {
            let name = entry.name.clone();
            let should_init = entry.core || enabled.contains(&name);

            if should_init {
                // Validate config requirements first
                if let Err(e) = registry::validate_config(&entry.config_reqs) {
                    if entry.core {
                        eprintln!("[WARN] [init] {name} config check failed: {e}");
                    } else {
                        eprintln!("[INFO] [init] {name}: {e}");
                    }
                    entry.status = ModuleStatus::Error(e);
                } else {
                    // Try to initialize
                    match (entry.init_fn)() {
                        Ok(()) => {
                            eprintln!("[INFO] [init] {name} initialized");
                            entry.status = ModuleStatus::Loaded;
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
                }
            }
            // Not enabled — keep as Unloaded (default status)
            (name, entry)
        })
        .collect();

    let (ok_count, total_count) = registry_map.values()
        .filter(|e| e.core || enabled.contains(&e.name))
        .fold((0usize, 0usize), |(ok, total), entry| {
            if matches!(entry.status, ModuleStatus::Loaded) {
                (ok + 1, total + 1)
            } else {
                (ok, total + 1)
            }
        });

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
