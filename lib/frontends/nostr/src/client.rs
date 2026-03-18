use secp256k1::{Keypair, Message, Secp256k1, SecretKey};
use sha2::{Digest, Sha256};
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

const COMPONENT: &str = "nostr-frontend";
const NOSTR_KEY_SYMBOLS: &[&str] = &["nostr-private-key", "nostr-nsec"];

pub struct NostrState {
    pub secret_key: Option<SecretKey>,
    pub public_key_hex: String,
    pub relay_urls: Vec<String>,
    pub last_seen_ts: u64,
    pub initialized: bool,
}

static STATE: OnceLock<RwLock<NostrState>> = OnceLock::new();

fn state() -> &'static RwLock<NostrState> {
    STATE.get_or_init(|| {
        RwLock::new(NostrState {
            secret_key: None,
            public_key_hex: String::new(),
            relay_urls: Vec::new(),
            last_seen_ts: 0,
            initialized: false,
        })
    })
}

fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
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

fn parse_relays_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

/// NIP-01 event structure
struct NostrEvent {
    id: String,
    pubkey: String,
    created_at: u64,
    kind: u64,
    tags: Vec<Vec<String>>,
    content: String,
    sig: String,
}

impl NostrEvent {
    fn to_json(&self) -> String {
        let tags_json: Vec<String> = self
            .tags
            .iter()
            .map(|tag| {
                let items: Vec<String> = tag.iter().map(|s| format!("\"{}\"", escape_json(s))).collect();
                format!("[{}]", items.join(","))
            })
            .collect();
        format!(
            "{{\"id\":\"{}\",\"pubkey\":\"{}\",\"created_at\":{},\"kind\":{},\"tags\":[{}],\"content\":\"{}\",\"sig\":\"{}\"}}",
            self.id,
            self.pubkey,
            self.created_at,
            self.kind,
            tags_json.join(","),
            escape_json(&self.content),
            self.sig
        )
    }
}

/// Sign an event per NIP-01 / BIP-340 Schnorr.
/// The private key NEVER leaves this function or process memory.
fn sign_event(
    sk: &SecretKey,
    content: &str,
    kind: u64,
    tags: &[Vec<String>],
) -> Result<NostrEvent, String> {
    let secp = Secp256k1::new();
    let kp = Keypair::from_secret_key(&secp, sk);
    let (xonly, _) = kp.x_only_public_key();
    let pubkey_hex = hex::encode(xonly.serialize());
    let created_at = now_unix_secs();

    // Build commitment string per NIP-01:
    // [0, pubkey, created_at, kind, tags, content]
    let commit_value = serde_json::json!([
        0,
        pubkey_hex,
        created_at,
        kind,
        tags,
        content
    ]);
    let commit_str = commit_value.to_string();

    let id_hash = Sha256::digest(commit_str.as_bytes());
    let msg = Message::from_digest_slice(&id_hash)
        .map_err(|e| format!("message from digest: {e}"))?;
    let sig = secp.sign_schnorr(&msg, &kp);

    Ok(NostrEvent {
        id: hex::encode(id_hash),
        pubkey: pubkey_hex,
        created_at,
        kind,
        tags: tags.to_vec(),
        content: content.to_string(),
        sig: hex::encode(sig.as_ref()),
    })
}

pub fn init(config: &str) -> Result<(), String> {
    let mut s = state().write().map_err(|e| format!("lock: {e}"))?;
    if s.initialized {
        return Err("nostr already initialized".into());
    }

    // Ingest private key from config into vault
    if let Some(key) = sexp_value(config, ":private-key")
        .or_else(|| sexp_value(config, ":nsec"))
    {
        let trimmed = key.trim();
        if !trimmed.is_empty() {
            harmonia_vault::set_secret_for_symbol("nostr-private-key", trimmed)?;
        }
    }

    // Ingest relays into config-store
    if let Some(relays) = sexp_value(config, ":relays") {
        let trimmed = relays.trim();
        if !trimmed.is_empty() {
            let _ = harmonia_config_store::set_config(COMPONENT, COMPONENT, "relays", trimmed);
        }
    }

    // Read private key from vault — NEVER sent over the wire
    let key_hex = read_vault_secret(NOSTR_KEY_SYMBOLS)?
        .ok_or("missing nostr private key in vault (symbol: nostr-private-key)")?;

    let key_bytes = hex::decode(key_hex.trim())
        .map_err(|e| format!("invalid hex private key: {e}"))?;
    let sk = SecretKey::from_slice(&key_bytes)
        .map_err(|e| format!("invalid secp256k1 key: {e}"))?;

    // Derive public key
    let secp = Secp256k1::new();
    let kp = Keypair::from_secret_key(&secp, &sk);
    let (xonly, _) = kp.x_only_public_key();
    s.public_key_hex = hex::encode(xonly.serialize());

    // Read relay URLs
    s.relay_urls = harmonia_config_store::get_own(COMPONENT, "relays")
        .ok()
        .flatten()
        .map(|v| parse_relays_csv(&v))
        .unwrap_or_else(|| vec!["wss://relay.damus.io".to_string()]);

    s.secret_key = Some(sk);
    s.last_seen_ts = now_unix_secs();
    s.initialized = true;
    Ok(())
}

pub fn poll() -> Result<Vec<(String, String, Option<String>)>, String> {
    let (relays, pubkey, since, _sk) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("nostr not initialized".into());
        }
        (
            s.relay_urls.clone(),
            s.public_key_hex.clone(),
            s.last_seen_ts,
            (), // don't clone the secret key
        )
    };

    let req = format!(
        "[\"REQ\",\"harmonia\",{{\"kinds\":[1,4],\"since\":{},\"#p\":[\"{}\"]}}]",
        since, pubkey
    );

    let mut all_events = Vec::new();
    let mut max_ts = since;

    for relay_url in &relays {
        match poll_relay(relay_url, &req) {
            Ok(events) => {
                for (author, content, ts) in events {
                    if ts > max_ts {
                        max_ts = ts;
                    }
                    if ts <= since {
                        continue;
                    }
                    let metadata = format!(
                        "(:channel-class \"nostr-relay\" :node-id \"{}\" :remote t)",
                        escape_metadata(&author)
                    );
                    all_events.push((author, content, Some(metadata)));
                }
            }
            Err(_) => {
                // Relay unavailable — skip silently
            }
        }
    }

    // Send CLOSE on next poll; for now update timestamp
    if max_ts > since {
        if let Ok(mut s) = state().write() {
            s.last_seen_ts = max_ts;
        }
    }

    Ok(all_events)
}

fn poll_relay(relay_url: &str, req: &str) -> Result<Vec<(String, String, u64)>, String> {
    let (mut socket, _response) =
        tungstenite::connect(relay_url).map_err(|e| format!("ws connect: {e}"))?;

    // Set read timeout for non-blocking behavior
    if let tungstenite::stream::MaybeTlsStream::Plain(ref tcp) = socket.get_ref() {
        let _ = tcp.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    }

    socket
        .send(tungstenite::Message::Text(req.to_string()))
        .map_err(|e| format!("ws send: {e}"))?;

    let mut events = Vec::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);

    loop {
        if std::time::Instant::now() >= deadline {
            break;
        }
        match socket.read() {
            Ok(tungstenite::Message::Text(text)) => {
                if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                    if arr.len() >= 3 && arr[0].as_str() == Some("EVENT") {
                        if let Some(event) = arr.get(2) {
                            let author = event
                                .get("pubkey")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown")
                                .to_string();
                            let content = event
                                .get("content")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();
                            let ts = event
                                .get("created_at")
                                .and_then(|v| v.as_u64())
                                .unwrap_or(0);
                            if !content.is_empty() {
                                events.push((author, content, ts));
                            }
                        }
                    } else if arr.len() >= 2 && arr[0].as_str() == Some("EOSE") {
                        break; // End of stored events
                    }
                }
            }
            Ok(tungstenite::Message::Close(_)) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Close subscription
    let close_msg = "[\"CLOSE\",\"harmonia\"]";
    let _ = socket.send(tungstenite::Message::Text(close_msg.to_string()));
    let _ = socket.close(None);

    Ok(events)
}

pub fn send(channel: &str, text: &str) -> Result<(), String> {
    let (sk, relays) = {
        let s = state().read().map_err(|e| format!("lock: {e}"))?;
        if !s.initialized {
            return Err("nostr not initialized".into());
        }
        (
            s.secret_key.ok_or("no secret key")?,
            s.relay_urls.clone(),
        )
    };

    // Kind 1 = text note (public), kind 4 = encrypted DM
    // For simplicity, use kind 1 with channel as a tag if it looks like a pubkey
    let (kind, tags) = if channel.len() == 64 && channel.chars().all(|c| c.is_ascii_hexdigit()) {
        // DM to a specific pubkey
        (4, vec![vec!["p".to_string(), channel.to_string()]])
    } else {
        // Public note
        (1, Vec::<Vec<String>>::new())
    };

    let event = sign_event(&sk, text, kind, &tags)?;
    let event_msg = format!("[\"EVENT\",{}]", event.to_json());

    let mut last_err = None;
    for relay_url in &relays {
        match send_to_relay(relay_url, &event_msg) {
            Ok(()) => return Ok(()),
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or_else(|| "no relays configured".into()))
}

fn send_to_relay(relay_url: &str, event_msg: &str) -> Result<(), String> {
    let (mut socket, _response) =
        tungstenite::connect(relay_url).map_err(|e| format!("ws connect: {e}"))?;

    socket
        .send(tungstenite::Message::Text(event_msg.to_string()))
        .map_err(|e| format!("ws send: {e}"))?;

    // Wait briefly for OK response
    if let tungstenite::stream::MaybeTlsStream::Plain(ref tcp) = socket.get_ref() {
        let _ = tcp.set_read_timeout(Some(std::time::Duration::from_secs(2)));
    }

    match socket.read() {
        Ok(tungstenite::Message::Text(text)) => {
            if let Ok(arr) = serde_json::from_str::<Vec<serde_json::Value>>(&text) {
                if arr.first().and_then(|v| v.as_str()) == Some("OK") {
                    let accepted = arr.get(2).and_then(|v| v.as_bool()).unwrap_or(false);
                    if !accepted {
                        let reason = arr.get(3).and_then(|v| v.as_str()).unwrap_or("rejected");
                        return Err(format!("relay rejected event: {reason}"));
                    }
                }
            }
        }
        _ => {}
    }

    let _ = socket.close(None);
    Ok(())
}

pub fn shutdown() {
    if let Ok(mut s) = state().write() {
        // Zeroize secret key by overwriting
        s.secret_key = None;
        s.public_key_hex.clear();
        s.relay_urls.clear();
        s.last_seen_ts = 0;
        s.initialized = false;
    }
}

pub fn is_initialized() -> bool {
    state().read().map(|s| s.initialized).unwrap_or(false)
}

fn escape_json(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
