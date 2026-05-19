//! [`EmailFrontend`] — actor-owned IMAP+SMTP frontend with PGP layer.

use async_trait::async_trait;

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

use crate::imap_io::{imap_get_latest_uid, imap_poll, ImapConfig};
use crate::parsing::{format_email_metadata, PgpVerifyState};
use crate::smtp_io::{smtp_send, SmtpConfig};

const COMPONENT: &str = "email-frontend";
const EMAIL_PASSWORD_SYMBOLS: &[&str] = &[
    "email-imap-password",
    "email-password",
    "email-smtp-password",
];
const EMAIL_SMTP_PASSWORD_SYMBOLS: &[&str] = &["email-smtp-password", "email-password"];

pub struct EmailFrontend {
    imap: ImapConfig,
    smtp: SmtpConfig,
    last_uid: u32,
}

fn config_or(key: &str, default: &str) -> String {
    harmonia_config_store::get_own(COMPONENT, key)
        .ok()
        .flatten()
        .unwrap_or_else(|| default.to_string())
}

fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
    harmonia_vault::init_from_env()?;
    for symbol in symbols {
        let maybe = harmonia_vault::get_secret_for_component(COMPONENT, symbol)
            .map_err(|e| format!("vault: {e}"))?;
        if let Some(value) = maybe {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return Ok(Some(trimmed.to_string()));
            }
        }
    }
    Ok(None)
}

#[async_trait]
impl Frontend for EmailFrontend {
    type Config = ();

    fn name() -> &'static str {
        "email"
    }

    async fn init(_: ()) -> Result<Self, String> {
        let imap_host = config_or("imap-host", "");
        let imap_port: u16 = config_or("imap-port", "993").parse().unwrap_or(993);
        let imap_user = config_or("imap-user", "");
        let imap_mailbox = config_or("imap-mailbox", "INBOX");
        let imap_use_tls = config_or("imap-tls", "true") != "false";
        let smtp_host = config_or("smtp-host", "");
        let smtp_port: u16 = config_or("smtp-port", "587").parse().unwrap_or(587);
        let smtp_user_cfg = config_or("smtp-user", "");
        let smtp_from_cfg = config_or("smtp-from", "");
        let smtp_tls = config_or("smtp-tls", "starttls");
        let default_subject = config_or("default-subject", "Harmonia message");

        let imap_password = read_vault_secret(EMAIL_PASSWORD_SYMBOLS)?.unwrap_or_default();
        let smtp_password = read_vault_secret(EMAIL_SMTP_PASSWORD_SYMBOLS)?
            .unwrap_or_else(|| imap_password.clone());

        if imap_host.is_empty() {
            return Err("email: imap-host is required".into());
        }
        if smtp_host.is_empty() {
            return Err("email: smtp-host is required".into());
        }
        if imap_password.is_empty() {
            return Err("email: missing IMAP password in vault".into());
        }

        let smtp_user = if smtp_user_cfg.is_empty() {
            imap_user.clone()
        } else {
            smtp_user_cfg
        };
        let smtp_from = if smtp_from_cfg.is_empty() {
            imap_user.clone()
        } else {
            smtp_from_cfg
        };

        let imap = ImapConfig {
            host: imap_host,
            port: imap_port,
            user: imap_user,
            password: imap_password,
            use_tls: imap_use_tls,
            mailbox: imap_mailbox,
        };
        let smtp = SmtpConfig {
            host: smtp_host,
            port: smtp_port,
            user: smtp_user,
            password: smtp_password,
            from: smtp_from,
            tls: smtp_tls,
            default_subject,
        };

        let imap_for_init = imap.clone();
        let last_uid = tokio::task::spawn_blocking(move || imap_get_latest_uid(&imap_for_init))
            .await
            .map_err(|e| format!("imap init join: {e}"))??;

        eprintln!(
            "[INFO] [email] frontend ready (imap={}:{} smtp={}:{})",
            imap.host, imap.port, smtp.host, smtp.port
        );

        Ok(Self {
            imap,
            smtp,
            last_uid,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let imap = self.imap.clone();
        let last_uid = self.last_uid;
        let fetched = tokio::task::spawn_blocking(move || imap_poll(&imap, last_uid))
            .await
            .map_err(|e| format!("imap poll join: {e}"))??;

        let mut out = Vec::new();
        for item in fetched {
            if item.uid > self.last_uid {
                self.last_uid = item.uid;
            }
            // Inline clearsigned-block detection. Full crypto verify lives
            // on the trust-store actor's future `VerifyClearsigned` variant;
            // until that's wired we mark detected blocks as
            // SignedUntrusted so policy stays conservative.
            let pgp_state = if item.body.contains("-----BEGIN PGP SIGNED MESSAGE-----") {
                PgpVerifyState::SignedUntrusted(String::new())
            } else {
                PgpVerifyState::Unsigned
            };
            let metadata = format_email_metadata(&item.sender, &item.headers, pgp_state);
            out.push(InboundMessage {
                address: item.sender,
                text: item.body,
                metadata: Some(metadata),
            });
        }
        Ok(out)
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        let smtp = self.smtp.clone();
        let to = channel.to_string();
        let body = text.to_string();
        tokio::task::spawn_blocking(move || smtp_send(&smtp, &to, &body))
            .await
            .map_err(|e| format!("smtp send join: {e}"))??;
        Ok(())
    }

    async fn shutdown(&mut self) {
        // Nothing to release — IMAP/SMTP are short-lived per-call.
    }
}
