use std::sync::{OnceLock, RwLock};

pub(crate) const COMPONENT: &str = "email-frontend";
pub(crate) const EMAIL_PASSWORD_SYMBOLS: &[&str] = &[
    "email-imap-password",
    "email-password",
    "email-smtp-password",
    "email-api-key",
];
pub(crate) const EMAIL_SMTP_PASSWORD_SYMBOLS: &[&str] = &[
    "email-smtp-password",
    "email-imap-password",
    "email-password",
    "email-api-key",
];

pub struct EmailState {
    pub imap_host: String,
    pub imap_port: u16,
    pub imap_user: String,
    pub imap_password: String,
    pub imap_tls: bool,
    pub imap_mailbox: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_user: String,
    pub smtp_password: String,
    pub smtp_from: String,
    pub smtp_tls: String, // "starttls" | "tls" | "none"
    pub last_uid: u32,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<EmailState>> = OnceLock::new();

pub(crate) fn state() -> &'static RwLock<EmailState> {
    STATE.get_or_init(|| {
        RwLock::new(EmailState {
            imap_host: String::new(),
            imap_port: 993,
            imap_user: String::new(),
            imap_password: String::new(),
            imap_tls: true,
            imap_mailbox: "INBOX".to_string(),
            smtp_host: String::new(),
            smtp_port: 587,
            smtp_user: String::new(),
            smtp_password: String::new(),
            smtp_from: String::new(),
            smtp_tls: "starttls".to_string(),
            last_uid: 0,
            initialized: false,
        })
    })
}

pub(crate) fn sexp_value(config: &str, key: &str) -> Option<String> {
    harmonia_actor_protocol::extract_sexp_string(config, key)
}

pub(crate) fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
    harmonia_vault::init_from_env()?;
    for symbol in symbols {
        let maybe = harmonia_vault::get_secret_for_component(COMPONENT, symbol)
            .map_err(|e| format!("vault policy error: {e}"))?;
        if let Some(value) = maybe {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }
    }
    Ok(None)
}

pub(crate) fn config_or(key: &str, default: &str) -> String {
    harmonia_config_store::get_own(COMPONENT, key)
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

pub fn is_initialized() -> bool {
    state().read().map(|s| s.initialized).unwrap_or(false)
}

pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        s.imap_host.clear();
        s.imap_password.clear();
        s.smtp_password.clear();
        s.last_uid = 0;
        s.initialized = false;
    }
}
