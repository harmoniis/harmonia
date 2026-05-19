//! Plain-TCP SIP transport. The agent opens one long-lived TCP stream
//! to `kamailio.service.consul:5060` and uses it as a bidirectional
//! request/response bus. The kamailio jail is reachable only from
//! sibling pots over the cluster LAN, so plain TCP is sufficient — no
//! TLS layer.

use std::sync::Arc;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::Mutex;

#[derive(Clone)]
pub(crate) struct SipTcpConfig {
    pub host: String,
    pub port: u16,
}

pub(crate) struct SipConnection {
    pub(crate) stream: Arc<Mutex<TcpStream>>,
}

pub(crate) async fn connect(config: &SipTcpConfig) -> Result<SipConnection, String> {
    let stream = TcpStream::connect((config.host.as_str(), config.port))
        .await
        .map_err(|e| format!("sip tcp connect {}:{}: {e}", config.host, config.port))?;
    Ok(SipConnection {
        stream: Arc::new(Mutex::new(stream)),
    })
}

impl SipConnection {
    pub(crate) async fn send_request(&self, request: &str) -> Result<(), String> {
        let mut guard = self.stream.lock().await;
        guard
            .write_all(request.as_bytes())
            .await
            .map_err(|e| format!("sip write: {e}"))?;
        guard
            .flush()
            .await
            .map_err(|e| format!("sip flush: {e}"))?;
        Ok(())
    }

    /// Read whatever the server has sent. Bounded to ~50 ms per call so
    /// the actor's `poll` doesn't block when the line is quiet.
    pub(crate) async fn read_available(&self) -> Result<Option<String>, String> {
        let mut buf = vec![0u8; 8192];
        let mut guard = self.stream.lock().await;
        match tokio::time::timeout(std::time::Duration::from_millis(50), guard.read(&mut buf))
            .await
        {
            Ok(Ok(0)) => Ok(None),
            Ok(Ok(n)) => Ok(Some(String::from_utf8_lossy(&buf[..n]).into_owned())),
            Ok(Err(e)) => Err(format!("sip read: {e}")),
            Err(_) => Ok(None),
        }
    }

    /// Block-read until either a full SIP response is buffered or the
    /// 2-second timeout fires. Used during the REGISTER handshake to
    /// pick up the 401 challenge before issuing the second REGISTER.
    pub(crate) async fn read_response_with_timeout(
        &self,
        deadline: std::time::Duration,
    ) -> Result<String, String> {
        let mut acc = String::new();
        let start = std::time::Instant::now();
        while start.elapsed() < deadline {
            if let Some(chunk) = self.read_available().await? {
                acc.push_str(&chunk);
                if acc.contains("\r\n\r\n") {
                    return Ok(acc);
                }
            } else {
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        }
        if acc.is_empty() {
            Err("sip response timeout".to_string())
        } else {
            Ok(acc)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InboundSipMessage {
    pub from: String,
    pub body: String,
}

/// Best-effort parser for inbound MESSAGE bodies. Splits a TCP read
/// chunk on `MESSAGE` request-line boundaries; each chunk yields one
/// `(from-uri, body)` pair.
pub(crate) fn parse_inbound_messages(raw: &str) -> Vec<InboundSipMessage> {
    let mut out = Vec::new();
    for chunk in raw.split("MESSAGE sip:").skip(1) {
        let mut from = String::new();
        let mut body = String::new();
        let mut in_body = false;
        for line in chunk.lines() {
            if in_body {
                if !body.is_empty() {
                    body.push('\n');
                }
                body.push_str(line);
                continue;
            }
            if line.is_empty() {
                in_body = true;
                continue;
            }
            if let Some(rest) = line.strip_prefix("From: ") {
                from = rest.trim().to_string();
            }
        }
        if !body.is_empty() {
            out.push(InboundSipMessage { from, body });
        }
    }
    out
}

/// Parse an inbound `WWW-Authenticate: Digest …` challenge into its
/// `(realm, nonce)` pair. Returns `None` when the input doesn't look
/// like a 401/407 challenge response.
pub(crate) fn parse_challenge(raw: &str) -> Option<(String, String)> {
    if !raw.contains("SIP/2.0 401") && !raw.contains("SIP/2.0 407") {
        return None;
    }
    let line = raw
        .lines()
        .find(|l| l.starts_with("WWW-Authenticate:") || l.starts_with("Proxy-Authenticate:"))?;
    let realm = extract_kv(line, "realm")?;
    let nonce = extract_kv(line, "nonce")?;
    Some((realm, nonce))
}

fn extract_kv(line: &str, key: &str) -> Option<String> {
    let needle = format!("{key}=\"");
    let start = line.find(&needle)? + needle.len();
    let rest = &line[start..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}
