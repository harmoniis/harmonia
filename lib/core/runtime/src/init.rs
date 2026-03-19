//! Component initialization — starts all frontends, backends, and tools.
//!
//! Each component is initialized with graceful degradation: if a component
//! fails to start (missing credentials, network issues, etc.), it logs a
//! warning and continues. The system operates in degraded mode rather than
//! failing to start entirely.

use std::ffi::CString;

/// Initialize all components. Called once at runtime startup.
/// Returns (initialized_count, total_count).
pub fn init_all() -> (usize, usize) {
    let mut ok = 0usize;
    let mut total = 0usize;

    // ── Core ─────────────────────────────────────────────────────────

    total += 1;
    match harmonia_config_store::init() {
        Ok(_) => {
            eprintln!("[INFO] [init] config-store initialized");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] config-store failed: {e}"),
    }

    total += 1;
    match harmonia_chronicle::init() {
        Ok(_) => {
            eprintln!("[INFO] [init] chronicle initialized");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] chronicle failed: {e}"),
    }

    total += 1;
    match harmonia_vault::init_from_env() {
        Ok(_) => {
            eprintln!("[INFO] [init] vault initialized");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] vault failed: {e}"),
    }

    // Memory
    total += 1;
    let memory_path = memory_db_path();
    let c_path = CString::new(memory_path.as_str()).unwrap_or_default();
    let rc = harmonia_memory::harmonia_memory_init(c_path.as_ptr());
    if rc == 0 {
        eprintln!("[INFO] [init] memory initialized");
        ok += 1;
    } else {
        eprintln!("[WARN] [init] memory init returned {rc}");
    }

    // Signalograd
    total += 1;
    let rc = harmonia_signalograd::harmonia_signalograd_init();
    if rc == 0 {
        eprintln!("[INFO] [init] signalograd initialized");
        ok += 1;
    } else {
        eprintln!("[WARN] [init] signalograd init returned {rc}");
    }

    // Harmonic matrix
    total += 1;
    match harmonia_harmonic_matrix::runtime::store::init() {
        Ok(_) => {
            eprintln!("[INFO] [init] harmonic-matrix initialized");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] harmonic-matrix failed: {e}"),
    }

    // ── TUI ──────────────────────────────────────────────────────────

    total += 1;
    match harmonia_tui::terminal::init() {
        Ok(()) => {
            eprintln!("[INFO] [init] tui session listener started");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] tui session listener failed: {e}"),
    }

    // ── Frontends (each gracefully degrades if credentials missing) ──

    init_frontend(
        "telegram",
        || harmonia_telegram::bot::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "slack",
        || harmonia_slack::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "discord",
        || harmonia_discord::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "signal",
        || harmonia_signal::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "mattermost",
        || harmonia_mattermost::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "nostr",
        || harmonia_nostr::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "email-client",
        || harmonia_email_client::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "whatsapp",
        || harmonia_whatsapp::client::init("()"),
        &mut ok,
        &mut total,
    );
    init_frontend(
        "tailscale",
        || harmonia_tailscale_frontend::bridge::init("()"),
        &mut ok,
        &mut total,
    );

    // imessage is macOS only
    #[cfg(target_os = "macos")]
    init_frontend(
        "imessage",
        || harmonia_imessage::client::init("()"),
        &mut ok,
        &mut total,
    );

    // ── Backends ─────────────────────────────────────────────────────

    total += 1;
    let rc = harmonia_provider_router::harmonia_provider_router_init();
    if rc == 0 {
        eprintln!("[INFO] [init] provider-router initialized");
        ok += 1;
    } else {
        eprintln!("[WARN] [init] provider-router init returned {rc}");
    }

    total += 1;
    match harmonia_voice_router::init() {
        Ok(_) => {
            eprintln!("[INFO] [init] voice-router initialized");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] voice-router failed: {e}"),
    }

    // ── Tailnet ──────────────────────────────────────────────────────

    total += 1;
    match harmonia_tailnet::transport::start_listener() {
        Ok(_) => {
            eprintln!("[INFO] [init] tailnet listener started");
            ok += 1;
        }
        Err(e) => eprintln!("[WARN] [init] tailnet listener failed: {e}"),
    }

    eprintln!("[INFO] [init] Initialization complete: {ok}/{total} components ready");
    (ok, total)
}

fn init_frontend(
    name: &str,
    f: impl FnOnce() -> Result<(), String>,
    ok: &mut usize,
    total: &mut usize,
) {
    *total += 1;
    match f() {
        Ok(()) => {
            eprintln!("[INFO] [init] {name} frontend initialized");
            *ok += 1;
        }
        Err(e) => {
            eprintln!("[WARN] [init] {name} frontend: {e}");
        }
    }
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
