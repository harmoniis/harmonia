use crate::parsing::{parse_body_text, parse_payment_headers, EmailPaymentHeaders};
use crate::state::state;

pub(crate) fn imap_get_latest_uid(
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
            let metadata = crate::parsing::format_email_metadata(&sender, &headers);
            (sender, body, Some(metadata))
        })
        .collect())
}
