//! Module registry — self-describing modules with config validation.
//!
//! Each module declares its name, config requirements, core status,
//! and init/shutdown functions. The registry is the single source of
//! truth for what modules exist and what they need to run.

use std::fmt;

/// Status of a module in the registry.
#[derive(Clone, Debug)]
pub enum ModuleStatus {
    Unloaded,
    Loaded,
    Error(String),
}

impl fmt::Display for ModuleStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ModuleStatus::Unloaded => write!(f, "unloaded"),
            ModuleStatus::Loaded => write!(f, "loaded"),
            ModuleStatus::Error(e) => write!(f, "error: {}", e),
        }
    }
}

/// A configuration requirement for a module.
#[derive(Clone, Debug)]
pub enum ConfigReq {
    /// A vault secret must exist under this symbol.
    VaultSecret(String),
    /// A config-store key must exist.
    ConfigKey { component: String, key: String },
}

impl fmt::Display for ConfigReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConfigReq::VaultSecret(s) => write!(f, "vault \"{}\"", s),
            ConfigReq::ConfigKey { component, key } => {
                write!(f, "config \"{}/{}\"", component, key)
            }
        }
    }
}

/// A module entry in the registry.
pub struct ModuleEntry {
    pub name: String,
    pub status: ModuleStatus,
    /// Core modules cannot be unloaded.
    pub core: bool,
    pub config_reqs: Vec<ConfigReq>,
    pub init_fn: fn() -> Result<(), String>,
    pub shutdown_fn: fn(),
}

/// Validate that all config requirements for a module are met.
pub fn validate_config(reqs: &[ConfigReq]) -> Result<(), String> {
    for req in reqs {
        match req {
            ConfigReq::VaultSecret(symbol) => {
                if !harmonia_vault::has_secret_for_symbol(symbol) {
                    return Err(format!("missing config: vault secret '{}'", symbol));
                }
            }
            ConfigReq::ConfigKey { component, key } => {
                // Try reading with the component as reader — this respects
                // config-store access policy. If policy denies, fall through
                // and let the module's init_fn handle the check internally.
                match harmonia_config_store::get_config(component, "default", key) {
                    Ok(Some(_)) => {}
                    Ok(None) => {
                        return Err(format!(
                            "missing config: config-store key '{}/{}'",
                            component, key
                        ));
                    }
                    Err(e) if e.contains("policy denied") => {
                        // Policy prevents cross-component reads — skip validation
                        // and let the module's own init_fn check internally.
                    }
                    Err(e) => {
                        return Err(format!(
                            "cannot check config key '{}/{}': {}",
                            component, key, e
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

/// No-op shutdown for modules that don't need explicit cleanup.
fn noop_shutdown() {}

/// Build the full module registry declaring every known module.
pub fn build_registry() -> Vec<ModuleEntry> {
    vec![
        // ── Core (always loaded, no config requirements) ──────────
        ModuleEntry {
            name: "config-store".into(),
            status: ModuleStatus::Unloaded,
            core: true,
            config_reqs: vec![],
            init_fn: || harmonia_config_store::init().map_err(|e| e.to_string()),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "chronicle".into(),
            status: ModuleStatus::Unloaded,
            core: true,
            config_reqs: vec![],
            init_fn: || harmonia_chronicle::init().map_err(|e| e.to_string()),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "vault".into(),
            status: ModuleStatus::Unloaded,
            core: true,
            config_reqs: vec![],
            init_fn: || harmonia_vault::init_from_env().map_err(|e| e.to_string()),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "memory".into(),
            status: ModuleStatus::Unloaded,
            core: true,
            config_reqs: vec![],
            init_fn: || {
                let path = crate::init::memory_db_path();
                let c = std::ffi::CString::new(path.as_str()).unwrap_or_default();
                let rc = harmonia_memory::harmonia_memory_init(c.as_ptr());
                if rc == 0 {
                    Ok(())
                } else {
                    Err(format!("memory init returned {rc}"))
                }
            },
            shutdown_fn: noop_shutdown,
        },
        // ── Optional (no config requirements) ─────────────────────
        ModuleEntry {
            name: "tui".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || harmonia_tui::terminal::init().map_err(|e| e),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "signalograd".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || {
                let rc = harmonia_signalograd::harmonia_signalograd_init();
                if rc == 0 {
                    Ok(())
                } else {
                    Err("signalograd init failed".into())
                }
            },
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "harmonic-matrix".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || harmonia_harmonic_matrix::runtime::store::init().map_err(|e| e.to_string()),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "observability".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::VaultSecret("langsmith-api-key".into())],
            init_fn: || {
                let rc = harmonia_observability::harmonia_observability_init();
                if rc == 0 { Ok(()) } else { Err("observability init failed".into()) }
            },
            shutdown_fn: noop_shutdown,
        },
        // ── Frontends (with config requirements) ──────────────────
        ModuleEntry {
            name: "telegram".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::VaultSecret("telegram-bot-token".into())],
            init_fn: || harmonia_telegram::bot::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "slack".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![
                ConfigReq::VaultSecret("slack-bot-token".into()),
                ConfigReq::VaultSecret("slack-app-token".into()),
            ],
            init_fn: || harmonia_slack::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "discord".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::VaultSecret("discord-bot-token".into())],
            init_fn: || harmonia_discord::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "signal".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::ConfigKey {
                component: "signal-frontend".into(),
                key: "account".into(),
            }],
            init_fn: || harmonia_signal::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "mattermost".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::VaultSecret("mattermost-bot-token".into())],
            init_fn: || harmonia_mattermost::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "nostr".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::VaultSecret("nostr-private-key".into())],
            init_fn: || harmonia_nostr::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "email".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::ConfigKey {
                component: "email-frontend".into(),
                key: "imap-host".into(),
            }],
            init_fn: || harmonia_email_client::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "whatsapp".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || harmonia_whatsapp::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        #[cfg(target_os = "macos")]
        ModuleEntry {
            name: "imessage".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::ConfigKey {
                component: "imessage-frontend".into(),
                key: "server-url".into(),
            }],
            init_fn: || harmonia_imessage::client::init("()"),
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "tailscale".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || harmonia_tailscale_frontend::bridge::init("()"),
            shutdown_fn: noop_shutdown,
        },
        // ── Backends ──────────────────────────────────────────────
        ModuleEntry {
            name: "provider-router".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![ConfigReq::VaultSecret("openrouter-api-key".into())],
            init_fn: || {
                let rc = harmonia_provider_router::harmonia_provider_router_init();
                if rc == 0 {
                    Ok(())
                } else {
                    Err("provider-router init failed".into())
                }
            },
            shutdown_fn: noop_shutdown,
        },
        ModuleEntry {
            name: "voice-router".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || harmonia_voice_router::init().map_err(|e| e.to_string()),
            shutdown_fn: noop_shutdown,
        },
        // ── Transport ─────────────────────────────────────────────
        ModuleEntry {
            name: "tailnet".into(),
            status: ModuleStatus::Unloaded,
            core: false,
            config_reqs: vec![],
            init_fn: || harmonia_tailnet::transport::start_listener().map_err(|e| e),
            shutdown_fn: noop_shutdown,
        },
    ]
}
