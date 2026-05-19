//! Gateway/baseband frontend config generation and runtime module detection.

pub(crate) fn generate_gateway_config(enabled: &[&str]) -> String {
    let so_ext = if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    };

    let mut entries = Vec::new();

    let all_frontends = [
        ("tui", "target/release/libharmonia_tui", ":owner", "nil"),
        (
            "mqtt",
            "target/release/libharmonia_mqtt",
            ":owner",
            "(:mqtt-broker-url :mqtt-cert)",
        ),
        (
            "http3",
            "target/release/libharmonia_http3",
            ":authenticated",
            "nil",
        ),
        (
            "whatsapp",
            "target/release/libharmonia_whatsapp",
            ":authenticated",
            "(:whatsapp-session)",
        ),
        (
            "telegram",
            "target/release/libharmonia_telegram",
            ":authenticated",
            "(:telegram-bot-token)",
        ),
        (
            "slack",
            "target/release/libharmonia_slack",
            ":authenticated",
            "(:slack-app-token :slack-bot-token)",
        ),
        (
            "discord",
            "target/release/libharmonia_discord",
            ":authenticated",
            "(:discord-bot-token)",
        ),
        (
            "signal",
            "target/release/libharmonia_signal",
            ":authenticated",
            "nil",
        ),
        (
            "tailscale",
            "target/release/libharmonia_tailscale_frontend",
            ":authenticated",
            "(:tailscale-auth-key)",
        ),
        (
            "email",
            "target/release/libharmonia_email",
            ":authenticated",
            "nil",
        ),
        (
            "sip",
            "target/release/libharmonia_sip",
            ":authenticated",
            "nil",
        ),
    ];

    for (name, path, label, keys) in &all_frontends {
        let auto_load = if *name == "signal" && enabled.contains(name) {
            ":if-ready"
        } else if enabled.contains(name) {
            "t"
        } else {
            "nil"
        };
        let mut extra = String::new();
        if *name == "signal" {
            extra.push_str("\n    :config-keys ((\"signal-frontend\" \"account\"))");
        }
        if *name == "http3" {
            extra.push_str(
                "\n    :config-keys ((\"http3-frontend\" \"bind\") (\"http3-frontend\" \"ca-cert\") (\"http3-frontend\" \"server-cert\") (\"http3-frontend\" \"server-key\") (\"http3-frontend\" \"trusted-client-fingerprints-json\"))",
            );
        }
        entries.push(format!(
            "   (:name \"{name}\"\n    :so-path \"{path}.{so_ext}\"\n    :security-label {label}\n    :auto-load {auto_load}{extra}\n    :vault-keys {keys})",
        ));
    }

    format!("(:frontends\n  ({}\n  ))\n", entries.join("\n"))
}

/// Detect which runtime modules have their config requirements satisfied
/// and return the list of module names that should be auto-enabled.
pub(crate) fn resolve_configured_modules() -> Vec<String> {
    let mut enabled: Vec<String> = vec![
        "tui",
        "signalograd",
        "harmonic-matrix",
        "observability",
        "whatsapp",
        "tailscale",
        "voice-router",
        "tailnet",
    ]
    .into_iter()
    .map(String::from)
    .collect();

    let vault_modules: &[(&str, &[&str])] = &[
        ("telegram", &["telegram-bot-token"]),
        ("slack", &["slack-bot-token", "slack-app-token"]),
        ("discord", &["discord-bot-token"]),
        ("provider-router", &["openrouter-api-key"]),
    ];

    for (module, secrets) in vault_modules {
        let all_present = secrets
            .iter()
            .all(|s| harmonia_vault::has_secret_for_symbol(s));
        if all_present {
            enabled.push(module.to_string());
        }
    }

    let config_modules: &[(&str, &str, &str)] = &[
        ("signal", "signal-frontend", "account"),
        ("email", "email-frontend", "imap-host"),
    ];

    for (module, component, key) in config_modules {
        if let Ok(Some(_)) = harmonia_config_store::get_config(component, "default", key) {
            enabled.push(module.to_string());
        }
    }

    enabled.sort();
    enabled.dedup();
    enabled
}
