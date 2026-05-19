//! Synchronous IMAP fetch (called from the actor's poll via spawn_blocking).

use crate::parsing::{parse_body_text, parse_payment_headers, EmailPaymentHeaders};

#[derive(Debug, Clone)]
pub(crate) struct ImapFetched {
    pub uid: u32,
    pub sender: String,
    pub body: String,
    pub headers: EmailPaymentHeaders,
}

#[derive(Clone)]
pub(crate) struct ImapConfig {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub use_tls: bool,
    pub mailbox: String,
}

pub(crate) fn imap_get_latest_uid(cfg: &ImapConfig) -> Result<u32, String> {
    if cfg.use_tls {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| format!("tls: {e}"))?;
        let client = imap::connect((cfg.host.as_str(), cfg.port), &cfg.host, &tls)
            .map_err(|e| format!("imap connect: {e}"))?;
        let mut session = client
            .login(&cfg.user, &cfg.password)
            .map_err(|(e, _)| format!("imap login: {e}"))?;
        let mbox = session
            .select(&cfg.mailbox)
            .map_err(|e| format!("imap select: {e}"))?;
        let uid = mbox.uid_next.unwrap_or(1).saturating_sub(1);
        let _ = session.logout();
        Ok(uid)
    } else {
        let tcp = std::net::TcpStream::connect((cfg.host.as_str(), cfg.port))
            .map_err(|e| format!("imap tcp: {e}"))?;
        let client = imap::Client::new(tcp);
        let mut session = client
            .login(&cfg.user, &cfg.password)
            .map_err(|(e, _)| format!("imap login: {e}"))?;
        let mbox = session
            .select(&cfg.mailbox)
            .map_err(|e| format!("imap select: {e}"))?;
        let uid = mbox.uid_next.unwrap_or(1).saturating_sub(1);
        let _ = session.logout();
        Ok(uid)
    }
}

pub(crate) fn imap_poll(cfg: &ImapConfig, last_uid: u32) -> Result<Vec<ImapFetched>, String> {
    if cfg.use_tls {
        let tls = native_tls::TlsConnector::builder()
            .build()
            .map_err(|e| format!("tls: {e}"))?;
        let client = imap::connect((cfg.host.as_str(), cfg.port), &cfg.host, &tls)
            .map_err(|e| format!("imap connect: {e}"))?;
        let mut session = client
            .login(&cfg.user, &cfg.password)
            .map_err(|(e, _)| format!("imap login: {e}"))?;
        session
            .select(&cfg.mailbox)
            .map_err(|e| format!("imap select: {e}"))?;
        let results = fetch_new(&mut session, last_uid)?;
        let _ = session.logout();
        Ok(results)
    } else {
        let tcp = std::net::TcpStream::connect((cfg.host.as_str(), cfg.port))
            .map_err(|e| format!("imap tcp: {e}"))?;
        let client = imap::Client::new(tcp);
        let mut session = client
            .login(&cfg.user, &cfg.password)
            .map_err(|(e, _)| format!("imap login: {e}"))?;
        session
            .select(&cfg.mailbox)
            .map_err(|e| format!("imap select: {e}"))?;
        let results = fetch_new(&mut session, last_uid)?;
        let _ = session.logout();
        Ok(results)
    }
}

fn fetch_new<T: std::io::Read + std::io::Write>(
    session: &mut imap::Session<T>,
    last_uid: u32,
) -> Result<Vec<ImapFetched>, String> {
    let search_range = format!("{}:*", last_uid + 1);
    let uids = session
        .uid_search(&search_range)
        .map_err(|e| format!("imap uid search: {e}"))?;

    let mut out = Vec::new();
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
                out.push(ImapFetched {
                    uid,
                    sender,
                    body,
                    headers,
                });
            }
        }
    }
    Ok(out)
}
