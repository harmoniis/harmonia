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
            "target/release/libharmonia_mqtt_client",
            ":authenticated",
            "(:mqtt-broker-url :mqtt-cert)",
        ),
        (
            "http2",
            "target/release/libharmonia_http2_mtls",
            ":authenticated",
            "nil",
        ),
        (
            "imessage",
            "target/release/libharmonia_imessage",
            ":authenticated",
            "(:bluebubbles-server-url :bluebubbles-password)",
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
            "target/release/libharmonia_email_client",
            ":authenticated",
            "nil",
        ),
        (
            "mattermost",
            "target/release/libharmonia_mattermost",
            ":authenticated",
            "nil",
        ),
        (
            "nostr",
            "target/release/libharmonia_nostr",
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
        if *name == "http2" {
            extra.push_str(
                "\n    :config-keys ((\"http2-frontend\" \"bind\") (\"http2-frontend\" \"ca-cert\") (\"http2-frontend\" \"server-cert\") (\"http2-frontend\" \"server-key\") (\"http2-frontend\" \"trusted-client-fingerprints-json\"))",
            );
        }
        if *name == "imessage" {
            extra.push_str("\n    :platforms (:macos)");
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
        ("mattermost", &["mattermost-bot-token"]),
        ("nostr", &["nostr-private-key"]),
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

    #[cfg(target_os = "macos")]
    {
        let macos_config_modules: &[(&str, &str, &str)] =
            &[("imessage", "imessage-frontend", "server-url")];
        for (module, component, key) in macos_config_modules {
            if let Ok(Some(_)) = harmonia_config_store::get_config(component, "default", key) {
                enabled.push(module.to_string());
            }
        }
    }

    for (module, component, key) in config_modules {
        if let Ok(Some(_)) = harmonia_config_store::get_config(component, "default", key) {
            enabled.push(module.to_string());
        }
    }

    enabled.sort();
    enabled.dedup();
    enabled
}
