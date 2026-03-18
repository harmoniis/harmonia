use mailparse::MailHeaderMap;
use std::sync::{OnceLock, RwLock};

const COMPONENT: &str = "email-frontend";
const EMAIL_PASSWORD_SYMBOLS: &[&str] = &[
    "email-imap-password",
    "email-password",
    "email-smtp-password",
    "email-api-key",
];
const EMAIL_SMTP_PASSWORD_SYMBOLS: &[&str] = &[
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

fn state() -> &'static RwLock<EmailState> {
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

fn sexp_value(config: &str, key: &str) -> Option<String> {
    let idx = config.find(key)?;
    let rest = &config[idx + key.len()..];
    let rest = rest.trim_start();
    if rest.starts_with('"') {
        let inner = &rest[1..];
        let end = inner.find('"')?;
        Some(inner[..end].to_string())
    } else {
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(rest.len());
        Some(rest[..end].to_string())
    }
}

fn read_vault_secret(symbols: &[&str]) -> Result<Option<String>, String> {
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

fn config_or(key: &str, default: &str) -> String {
    harmonia_config_store::get_own(COMPONENT, key)
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| default.to_string())
}

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
    let last_uid = imap_get_latest_uid(
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

fn imap_get_latest_uid(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    use_tls: bool,
    mailbox: &str,
) -> Result<u32, String> {
    if use_tls {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| format!("tls: {e}"))?;
        let client =
            imap::connect((host, port), host, &tls).map_err(|e| format!("imap connect: {e}"))?;
        let mut session = client
            .login(user, password)
            .map_err(|(e, _)| format!("imap login: {e}"))?;
        let mbox = session
            .select(mailbox)
            .map_err(|e| format!("imap select: {e}"))?;
        let uid = mbox.uid_next.unwrap_or(1).saturating_sub(1);
        let _ = session.logout();
        Ok(uid)
    } else {
        let tcp =
            std::net::TcpStream::connect((host, port)).map_err(|e| format!("imap tcp: {e}"))?;
        let client = imap::Client::new(tcp);
        let mut session = client
            .login(user, password)
            .map_err(|(e, _)| format!("imap login: {e}"))?;
        let mbox = session
            .select(mailbox)
            .map_err(|e| format!("imap select: {e}"))?;
        let uid = mbox.uid_next.unwrap_or(1).saturating_sub(1);
        let _ = session.logout();
        Ok(uid)
    }
}

pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (host, port, user, password, use_tls, mailbox, last_uid) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("email not initialized".into());
        }
        (
            s.imap_host.clone(),
            s.imap_port,
            s.imap_user.clone(),
            s.imap_password.clone(),
            s.imap_tls,
            s.imap_mailbox.clone(),
            s.last_uid,
        )
    };

    let results = if use_tls {
        imap_poll_tls(&host, port, &user, &password, &mailbox, last_uid)?
    } else {
        imap_poll_plain(&host, port, &user, &password, &mailbox, last_uid)?
    };

    // Update last_uid
    if !results.is_empty() {
        if let Ok(mut s) = state().write() {
            for item in &results {
                let uid = item.0;
                if uid > s.last_uid {
                    s.last_uid = uid;
                }
            }
        }
    }

    Ok(results
        .into_iter()
        .map(|(_, sender, body, headers)| {
            let metadata = format_email_metadata(&sender, &headers);
            (sender, body, Some(metadata))
        })
        .collect())
}

fn imap_poll_tls(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    mailbox: &str,
    last_uid: u32,
) -> Result<Vec<(u32, String, String, EmailPaymentHeaders)>, String> {
    let tls = native_tls::TlsConnector::builder()
        .build()
        .map_err(|e| format!("tls: {e}"))?;
    let client =
        imap::connect((host, port), host, &tls).map_err(|e| format!("imap connect: {e}"))?;
    let mut session = client
        .login(user, password)
        .map_err(|(e, _)| format!("imap login: {e}"))?;
    session
        .select(mailbox)
        .map_err(|e| format!("imap select: {e}"))?;

    let results = fetch_new_messages(&mut session, last_uid)?;
    let _ = session.logout();
    Ok(results)
}

fn imap_poll_plain(
    host: &str,
    port: u16,
    user: &str,
    password: &str,
    mailbox: &str,
    last_uid: u32,
) -> Result<Vec<(u32, String, String, EmailPaymentHeaders)>, String> {
    let tcp = std::net::TcpStream::connect((host, port)).map_err(|e| format!("imap tcp: {e}"))?;
    let client = imap::Client::new(tcp);
    let mut session = client
        .login(user, password)
        .map_err(|(e, _)| format!("imap login: {e}"))?;
    session
        .select(mailbox)
        .map_err(|e| format!("imap select: {e}"))?;

    let results = fetch_new_messages(&mut session, last_uid)?;
    let _ = session.logout();
    Ok(results)
}

fn fetch_new_messages<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
    last_uid: u32,
) -> Result<Vec<(u32, String, String, EmailPaymentHeaders)>, String> {
    let search_range = format!("{}:*", last_uid + 1);
    let uids = session
        .uid_search(&search_range)
        .map_err(|e| format!("imap uid search: {e}"))?;

    let mut results = Vec::new();
    for uid in uids {
        if uid <= last_uid {
            continue;
        }
        let fetch_result = session
            .uid_fetch(uid.to_string(), "(ENVELOPE BODY.PEEK[HEADER] BODY[TEXT])")
            .map_err(|e| format!("imap uid fetch: {e}"))?;

        for msg in fetch_result.iter() {
            let envelope = msg.envelope();
            let sender = envelope
                .and_then(|env| env.from.as_ref())
                .and_then(|addrs| addrs.first())
                .map(|addr| {
                    let mailbox = addr
                        .mailbox
                        .as_ref()
                        .map(|s| std::str::from_utf8(s).unwrap_or(""))
                        .unwrap_or("");
                    let host = addr
                        .host
                        .as_ref()
                        .map(|s| std::str::from_utf8(s).unwrap_or(""))
                        .unwrap_or("");
                    if host.is_empty() {
                        mailbox.to_string()
                    } else {
                        format!("{mailbox}@{host}")
                    }
                })
                .unwrap_or_else(|| "unknown".to_string());

            let headers = parse_payment_headers(msg.header().unwrap_or(b""));
            let body_raw = msg.text().or_else(|| msg.body()).unwrap_or(b"");
            let body = parse_body_text(body_raw);

            if !body.trim().is_empty() {
                results.push((uid, sender, body, headers));
            }
        }
    }

    Ok(results)
}

fn parse_body_text(raw: &[u8]) -> String {
    match mailparse::parse_mail(raw) {
        Ok(parsed) => {
            // Prefer text/plain subpart
            for sub in &parsed.subparts {
                if let Some(ct) = sub
                    .headers
                    .iter()
                    .find(|h| h.get_key_ref() == "Content-Type")
                {
                    if ct.get_value().contains("text/plain") {
                        if let Ok(body) = sub.get_body() {
                            return body;
                        }
                    }
                }
            }
            // Fall back to main body
            parsed
                .get_body()
                .unwrap_or_else(|_| String::from_utf8_lossy(raw).to_string())
        }
        Err(_) => String::from_utf8_lossy(raw).to_string(),
    }
}

pub fn send(to: &str, text: &str) -> Result<(), String> {
    let (from, smtp_host, smtp_port, smtp_user, smtp_password, smtp_tls, subject) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("email not initialized".into());
        }
        let subject = harmonia_config_store::get_own(COMPONENT, "default-subject")
            .ok()
            .flatten()
            .unwrap_or_else(|| "Harmonia message".to_string());
        (
            s.smtp_from.clone(),
            s.smtp_host.clone(),
            s.smtp_port,
            s.smtp_user.clone(),
            s.smtp_password.clone(),
            s.smtp_tls.clone(),
            subject,
        )
    };

    let email = lettre::Message::builder()
        .from(from.parse().map_err(|e| format!("from addr: {e}"))?)
        .to(to.parse().map_err(|e| format!("to addr: {e}"))?)
        .subject(subject)
        .body(text.to_string())
        .map_err(|e| format!("build email: {e}"))?;

    let creds = lettre::transport::smtp::authentication::Credentials::new(
        smtp_user.clone(),
        smtp_password.clone(),
    );

    match smtp_tls.as_str() {
        "tls" => {
            let transport = lettre::SmtpTransport::relay(&smtp_host)
                .map_err(|e| format!("smtp relay: {e}"))?
                .port(smtp_port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
        "none" => {
            let transport = lettre::SmtpTransport::builder_dangerous(&smtp_host)
                .port(smtp_port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
        _ => {
            // "starttls" (default)
            let transport = lettre::SmtpTransport::starttls_relay(&smtp_host)
                .map_err(|e| format!("smtp starttls: {e}"))?
                .port(smtp_port)
                .credentials(creds)
                .build();
            lettre::Transport::send(&transport, &email).map_err(|e| format!("smtp send: {e}"))?;
        }
    }

    Ok(())
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

pub fn is_initialized() -> bool {
    state().read().map(|s| s.initialized).unwrap_or(false)
}

fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

#[derive(Debug, Clone, Default)]
struct EmailPaymentHeaders {
    rail: Option<String>,
    proof: Option<String>,
    action: Option<String>,
    challenge: Option<String>,
}

fn parse_payment_headers(raw: &[u8]) -> EmailPaymentHeaders {
    let Ok((headers, _)) = mailparse::parse_headers(raw) else {
        return EmailPaymentHeaders::default();
    };
    EmailPaymentHeaders {
        rail: header_value(&headers, "X-Harmoniis-Payment-Rail"),
        proof: header_value(&headers, "X-Harmoniis-Payment-Proof"),
        action: header_value(&headers, "X-Harmoniis-Payment-Action"),
        challenge: header_value(&headers, "X-Harmoniis-Payment-Challenge"),
    }
}

fn header_value(headers: &[mailparse::MailHeader<'_>], name: &str) -> Option<String> {
    headers
        .get_first_value(name)
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn format_email_metadata(sender: &str, headers: &EmailPaymentHeaders) -> String {
    let mut metadata = vec![
        format!(":channel-class \"email-imap\""),
        format!(":node-id \"{}\"", escape_metadata(sender)),
        ":remote t".to_string(),
    ];
    if let Some(rail) = &headers.rail {
        metadata.push(format!(":payment-rail \"{}\"", escape_metadata(rail)));
    }
    if let Some(proof) = &headers.proof {
        metadata.push(format!(":payment-proof \"{}\"", escape_metadata(proof)));
    }
    if let Some(action) = &headers.action {
        metadata.push(format!(":payment-action \"{}\"", escape_metadata(action)));
    }
    if let Some(challenge) = &headers.challenge {
        metadata.push(format!(
            ":payment-challenge \"{}\"",
            escape_metadata(challenge)
        ));
    }
    format!("({})", metadata.join(" "))
}

#[cfg(test)]
mod tests {
    use super::{format_email_metadata, parse_payment_headers};

    #[test]
    fn parses_payment_headers_into_metadata_fields() {
        let raw = concat!(
            "From: payer@example.com\r\n",
            "X-Harmoniis-Payment-Rail: voucher\r\n",
            "X-Harmoniis-Payment-Proof: proof-123\r\n",
            "X-Harmoniis-Payment-Action: post\r\n",
            "X-Harmoniis-Payment-Challenge: challenge-42\r\n",
            "\r\n",
        );
        let headers = parse_payment_headers(raw.as_bytes());
        let metadata = format_email_metadata("payer@example.com", &headers);

        assert!(metadata.contains(":payment-rail \"voucher\""));
        assert!(metadata.contains(":payment-proof \"proof-123\""));
        assert!(metadata.contains(":payment-action \"post\""));
        assert!(metadata.contains(":payment-challenge \"challenge-42\""));
    }
}
