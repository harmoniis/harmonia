/// Native command handlers — executed entirely in Rust, no IPC needed.

fn kv(key: &str, value: &str) -> String {
    format!("  {:<24} {}", key, value)
}

// ── Tier commands ────────────────────────────────────────────────────

fn execute_tier_change(tier: &str) -> String {
    let _ = harmonia_config_store::set_config("router", "router", "active-tier", tier);
    format!("[system] Routing tier: {}", tier)
}

pub(crate) fn execute_tier_auto(_args: &str) -> String {
    execute_tier_change("auto")
}

pub(crate) fn execute_tier_eco(_args: &str) -> String {
    execute_tier_change("eco")
}

pub(crate) fn execute_tier_premium(_args: &str) -> String {
    execute_tier_change("premium")
}

pub(crate) fn execute_tier_free(_args: &str) -> String {
    execute_tier_change("free")
}

// ── /help ────────────────────────────────────────────────────────────

pub(crate) fn execute_help(_args: &str) -> String {
    let lines = vec![
        "Harmonia System Commands".to_string(),
        String::new(),
        "Lisp-backed (runtime state):".to_string(),
        kv("/status", "System status overview"),
        kv("/backends", "List configured LLM backends"),
        kv("/backends <name>", "Show specific backend details"),
        kv("/frontends", "List all frontends with status"),
        kv("/frontends <name>", "Show specific frontend details"),
        kv("/tools", "List configured tools"),
        kv("/chronicle", "Chronicle overview (summary + GC)"),
        kv("/chronicle harmony", "Harmony summary"),
        kv("/chronicle delegation", "Delegation report"),
        kv("/chronicle costs", "Cost report"),
        kv("/chronicle graph", "Concept graph overview"),
        kv("/chronicle gc", "GC status"),
        kv("/metrics", "Metrics overview (parallel report)"),
        kv("/security", "Security audit overview"),
        kv("/security posture", "Current posture details"),
        kv("/security errors", "Recent errors from error ring"),
        kv("/feedback <note>", "Record human feedback"),
        kv("/policies", "Channel sender policies"),
        kv("/exit", "Exit the TUI session (TUI only)"),
        String::new(),
        "Gateway-native (Rust):".to_string(),
        kv("/wallet", "Wallet/vault status"),
        kv("/identity", "Vault symbols and key status"),
        kv("/help", "Show this listing"),
        String::new(),
        "Routing (Owner/Authenticated):".to_string(),
        kv("/auto", "Intelligent routing (default)"),
        kv("/eco", "Cost-optimized routing"),
        kv("/premium", "Quality-optimized routing"),
        kv("/free", "Zero-cost routing (local CLI only)"),
        kv("/route", "Current routing status"),
    ];
    lines.join("\n")
}

// ── /wallet ──────────────────────────────────────────────────────────

pub(crate) fn execute_wallet(_args: &str) -> String {
    if let Err(e) = harmonia_vault::init_from_env() {
        return format!("[system] Vault initialization failed: {e}");
    }
    let wallet_db = harmonia_config_store::get_config("gateway", "global", "wallet-db")
        .ok()
        .flatten()
        .unwrap_or_default();
    let vault_db = harmonia_config_store::get_config("gateway", "global", "vault-db")
        .ok()
        .flatten()
        .unwrap_or_default();
    let wallet_present = !wallet_db.is_empty() && std::path::Path::new(&wallet_db).exists();
    let vault_present = !vault_db.is_empty() && std::path::Path::new(&vault_db).exists();
    let symbols = harmonia_vault::list_secret_symbols();

    let mut lines = vec![
        "Wallet".to_string(),
        "-".repeat(40),
        kv(
            "Wallet DB:",
            if wallet_db.is_empty() {
                "(not set)"
            } else {
                &wallet_db
            },
        ),
        kv("Wallet present:", if wallet_present { "yes" } else { "no" }),
        kv(
            "Vault DB:",
            if vault_db.is_empty() {
                "(not set)"
            } else {
                &vault_db
            },
        ),
        kv("Vault present:", if vault_present { "yes" } else { "no" }),
        kv("Symbols:", &symbols.len().to_string()),
    ];
    if !symbols.is_empty() {
        lines.push(String::new());
        for sym in &symbols {
            let present = harmonia_vault::has_secret_for_symbol(sym);
            lines.push(format!(
                "  {:<28} {}",
                sym,
                if present { "[set]" } else { "[empty]" }
            ));
        }
    }
    lines.join("\n")
}

// ── Session commands ─────────────────────────────────────────────────

pub(crate) fn execute_session_create(_args: &str) -> String {
    let data_dir = match crate::sessions::resolve_data_dir() {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let label = match crate::sessions::resolve_node_label() {
        Ok(l) => l,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    match crate::sessions::create(&label, &data_dir) {
        Ok(s) => serde_json::to_string(&s).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

pub(crate) fn execute_session_list(_args: &str) -> String {
    let data_dir = match crate::sessions::resolve_data_dir() {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let label = match crate::sessions::resolve_node_label() {
        Ok(l) => l,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    match crate::sessions::list(&label, &data_dir) {
        Ok(s) => serde_json::to_string(&s).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}")),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

pub(crate) fn execute_session_current(_args: &str) -> String {
    let data_dir = match crate::sessions::resolve_data_dir() {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let label = match crate::sessions::resolve_node_label() {
        Ok(l) => l,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    match crate::sessions::current(&label, &data_dir) {
        Ok(Some(s)) => {
            serde_json::to_string(&s).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
        Ok(None) => "{\"session\":null}".to_string(),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

pub(crate) fn execute_session_events(args: &str) -> String {
    let data_dir = match crate::sessions::resolve_data_dir() {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let label = match crate::sessions::resolve_node_label() {
        Ok(l) => l,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let session_id = args.trim();
    let session = if session_id.is_empty() {
        match crate::sessions::current(&label, &data_dir) {
            Ok(Some(s)) => s,
            Ok(None) => return "{\"events\":[]}".to_string(),
            Err(e) => return format!("{{\"error\":\"{e}\"}}"),
        }
    } else {
        match crate::sessions::resume(&label, &data_dir, session_id) {
            Ok(s) => s,
            Err(e) => return format!("{{\"error\":\"{e}\"}}"),
        }
    };
    match crate::sessions::read_events(&session) {
        Ok(events) => {
            serde_json::to_string(&events).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
        }
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

pub(crate) fn execute_session_append(args: &str) -> String {
    let data_dir = match crate::sessions::resolve_data_dir() {
        Ok(d) => d,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let label = match crate::sessions::resolve_node_label() {
        Ok(l) => l,
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    let parsed: Result<serde_json::Value, _> = serde_json::from_str(args);
    let (actor, kind, text) = match parsed {
        Ok(v) => {
            let a = v.get("actor").and_then(|x| x.as_str()).unwrap_or("system");
            let k = v.get("kind").and_then(|x| x.as_str()).unwrap_or("event");
            let t = v.get("text").and_then(|x| x.as_str()).unwrap_or("");
            (a.to_string(), k.to_string(), t.to_string())
        }
        Err(e) => return format!("{{\"error\":\"invalid JSON: {e}\"}}"),
    };
    let session = match crate::sessions::current(&label, &data_dir) {
        Ok(Some(s)) => s,
        Ok(None) => return "{\"error\":\"no active session\"}".to_string(),
        Err(e) => return format!("{{\"error\":\"{e}\"}}"),
    };
    match crate::sessions::append_event(&session, &actor, &kind, &text) {
        Ok(()) => "{\"ok\":true}".to_string(),
        Err(e) => format!("{{\"error\":\"{e}\"}}"),
    }
}

// ── /identity ────────────────────────────────────────────────────────

pub(crate) fn execute_identity(_args: &str) -> String {
    if let Err(e) = harmonia_vault::init_from_env() {
        return format!("[system] Vault initialization failed: {e}");
    }
    let symbols = harmonia_vault::list_secret_symbols();
    let mut lines = vec![
        "Identity & Vault".to_string(),
        "-".repeat(40),
        format!("Vault symbols ({}):", symbols.len()),
    ];
    if symbols.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for sym in &symbols {
            let present = harmonia_vault::has_secret_for_symbol(sym);
            lines.push(format!(
                "  {:<28} {}",
                sym,
                if present { "[set]" } else { "[empty]" }
            ));
        }
    }
    lines.push(String::new());
    lines.push("Backend key status:".to_string());
    for key_name in &["ANTHROPIC_API_KEY", "OPENROUTER_API_KEY"] {
        let has = harmonia_vault::has_secret_for_symbol(key_name);
        lines.push(format!(
            "  {:<28} {}",
            key_name,
            if has { "present" } else { "missing" }
        ));
    }
    lines.join("\n")
}
