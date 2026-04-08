use crate::imap;
use crate::state::{
    config_or, read_vault_secret, sexp_value, state, COMPONENT, EMAIL_PASSWORD_SYMBOLS,
    EMAIL_SMTP_PASSWORD_SYMBOLS,
};

pub use crate::imap::poll;
pub use crate::smtp::send;
pub use crate::state::{is_initialized, shutdown, EmailState};

pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;
    if s.initialized {
        return Err("email already initialized".into());
    }

    // Ingest s-expr config values into config-store/vault
    let sexp_keys = [
        (":imap-host", "imap-host"),
        (":imap-port", "imap-port"),
        (":imap-user", "imap-user"),
        (":imap-mailbox", "imap-mailbox"),
        (":imap-tls", "imap-tls"),
        (":smtp-host", "smtp-host"),
        (":smtp-port", "smtp-port"),
        (":smtp-user", "smtp-user"),
        (":smtp-from", "smtp-from"),
        (":smtp-tls", "smtp-tls"),
    ];
    for (sexp_key, store_key) in &sexp_keys {
        if let Some(val) = sexp_value(config, sexp_key) {
            let trimmed = val.trim();
            if !trimmed.is_empty() {
                let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, store_key, trimmed);
            }
        }
    }

    // Ingest passwords into vault
    if let Some(pw) = sexp_value(config, ":imap-password") {
        let trimmed = pw.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("email-imap-password", trimmed)?;
        }
    }
    if let Some(pw) = sexp_value(config, ":smtp-password") {
        let trimmed = pw.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("email-smtp-password", trimmed)?;
        }
    }
    // Legacy single password key
    if let Some(pw) = sexp_value(config, ":password") {
        let trimmed = pw.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("email-password", trimmed)?;
        }
    }

    // Read config-store values
    s.imap_host = config_or("imap-host", "");
    s.imap_port = config_or("imap-port", "993").parse().unwrap_or(993);
    s.imap_user = config_or("imap-user", "");
    s.imap_mailbox = config_or("imap-mailbox", "INBOX");
    s.imap_tls = config_or("imap-tls", "true") != "false";
    s.smtp_host = config_or("smtp-host", "");
    s.smtp_port = config_or("smtp-port", "587").parse().unwrap_or(587);
    s.smtp_user = config_or("smtp-user", "");
    s.smtp_from = config_or("smtp-from", "");
    s.smtp_tls = config_or("smtp-tls", "starttls");

    // Read passwords from vault
    s.imap_password = read_vault_secret(EMAIL_PASSWORD_SYMBOLS)?.unwrap_or_default();
    s.smtp_password = read_vault_secret(EMAIL_SMTP_PASSWORD_SYMBOLS)?.unwrap_or_default();

    // Validate minimum config
    if s.imap_host.is_empty() {
        return Err("email: imap-host is required".into());
    }
    if s.smtp_host.is_empty() {
        return Err("email: smtp-host is required".into());
    }
    if s.imap_password.is_empty() {
        return Err("email: missing IMAP password in vault".into());
    }

    // If smtp_user empty, fall back to imap_user
    if s.smtp_user.is_empty() {
        s.smtp_user = s.imap_user.clone();
    }
    // If smtp_password empty, fall back to imap_password
    if s.smtp_password.is_empty() {
        s.smtp_password = s.imap_password.clone();
    }
    // If smtp_from empty, fall back to imap_user
    if s.smtp_from.is_empty() {
        s.smtp_from = s.imap_user.clone();
    }

    // Connect to IMAP to establish last_uid (skip history)
    let last_uid = imap::imap_get_latest_uid(
        &s.imap_host,
        s.imap_port,
        &s.imap_user,
        &s.imap_password,
        s.imap_tls,
        &s.imap_mailbox,
    )?;
    s.last_uid = last_uid;
    s.initialized = true;
    Ok(())
}
