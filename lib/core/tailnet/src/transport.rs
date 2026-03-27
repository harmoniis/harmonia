use std::collections::VecDeque;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{OnceLock, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::mesh;
use crate::model::MeshMessage;
use crate::node_link;

/// Wave 4.2: Maximum age of a mesh message before it's rejected (replay protection).
const MAX_MESSAGE_AGE_MS: u64 = 5 * 60 * 1000; // 5 minutes

fn mesh_shared_secret() -> Option<String> {
    harmonia_config_store::get_config("tailnet-core", "tailnet-core", "shared-secret")
        .ok()
        .flatten()
        .filter(|s| !s.is_empty())
}

/// Wave 4.2: Validate HMAC and timestamp on incoming messages.
/// Returns Ok(()) if valid or no shared secret is configured.
fn validate_message(msg: &MeshMessage) -> Result<(), String> {
    // If no HMAC is present and no shared secret is configured, allow (backward compatible)
    let shared_secret = match mesh_shared_secret() {
        Some(v) => v,
        None => return Ok(()), // No secret configured, skip validation
    };

    // Require HMAC when a shared secret is configured
    if msg.hmac.is_empty() {
        return Err("mesh message missing HMAC (shared secret is configured)".to_string());
    }

    // Replay protection: reject messages older than MAX_MESSAGE_AGE_MS
    if msg.timestamp_ms > 0 {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let age = if now_ms > msg.timestamp_ms {
            now_ms - msg.timestamp_ms
        } else {
            0
        };
        if age > MAX_MESSAGE_AGE_MS {
            return Err(format!(
                "mesh message too old: {}ms (max {}ms)",
                age, MAX_MESSAGE_AGE_MS
            ));
        }
    }

    // Compute expected HMAC
    let data = format!(
        "{}|{}|{}|{}|{}",
        msg.from,
        msg.to,
        msg.payload,
        msg.msg_type.as_str(),
        msg.timestamp_ms
    );
    // HMAC-SHA256 using a simple implementation
    let expected = hmac_sha256_hex(shared_secret.as_bytes(), data.as_bytes());
    if !constant_time_eq(expected.as_bytes(), msg.hmac.as_bytes()) {
        return Err("mesh message HMAC verification failed".to_string());
    }

    Ok(())
}

fn sign_message_if_needed(msg: &mut MeshMessage) {
    let Some(shared_secret) = mesh_shared_secret() else {
        return;
    };
    if msg.timestamp_ms == 0 {
        msg.timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
    }
    let data = format!(
        "{}|{}|{}|{}|{}",
        msg.from,
        msg.to,
        msg.payload,
        msg.msg_type.as_str(),
        msg.timestamp_ms
    );
    msg.hmac = hmac_sha256_hex(shared_secret.as_bytes(), data.as_bytes());
}

/// Constant-time comparison to prevent timing attacks.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Simple HMAC-SHA256 using the standard approach: H((K xor opad) || H((K xor ipad) || message))
/// For production, consider using a proper HMAC crate.
fn hmac_sha256_hex(key: &[u8], message: &[u8]) -> String {
    use sha2::{Digest, Sha256};

    let block_size = 64;
    let mut key_block = vec![0u8; block_size];
    if key.len() > block_size {
        let hash = Sha256::digest(key);
        key_block[..32].copy_from_slice(&hash);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut ipad = vec![0x36u8; block_size];
    let mut opad = vec![0x5cu8; block_size];
    for i in 0..block_size {
        ipad[i] ^= key_block[i];
        opad[i] ^= key_block[i];
    }

    let mut inner_hasher = Sha256::new();
    inner_hasher.update(&ipad);
    inner_hasher.update(message);
    let inner_hash = inner_hasher.finalize();

    let mut outer_hasher = Sha256::new();
    outer_hasher.update(&opad);
    outer_hasher.update(&inner_hash);
    let result = outer_hasher.finalize();

    hex::encode(result)
}

struct TransportState {
    inbound_queue: VecDeque<MeshMessage>,
    listener_running: bool,
}

static TRANSPORT: OnceLock<RwLock<TransportState>> = OnceLock::new();
static STOP_FLAG: AtomicBool = AtomicBool::new(false);

fn transport() -> &'static RwLock<TransportState> {
    TRANSPORT.get_or_init(|| {
        RwLock::new(TransportState {
            inbound_queue: VecDeque::new(),
            listener_running: false,
        })
    })
}

/// Start a background listener thread on the configured port.
/// Incoming connections carry length-prefixed JSON messages.
pub fn start_listener() -> Result<(), String> {
    {
        let state = transport().read().map_err(|e| format!("lock: {}", e))?;
        if state.listener_running {
            return Ok(());
        }
    }

    let port = mesh::listen_port();
    let addr = format!("0.0.0.0:{}", port);
    let listener = TcpListener::bind(&addr).map_err(|e| format!("bind {}: {}", addr, e))?;

    // Non-blocking so we can check the stop flag periodically.
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set_nonblocking: {}", e))?;

    STOP_FLAG.store(false, Ordering::SeqCst);

    // Actor registration is now handled by the runtime IPC system.

    {
        let mut state = transport().write().map_err(|e| format!("lock: {}", e))?;
        state.listener_running = true;
    }

    std::thread::spawn(move || {
        log::info!("tailnet listener started on {}", addr);
        loop {
            if STOP_FLAG.load(Ordering::SeqCst) {
                break;
            }
            match listener.accept() {
                Ok((stream, _peer)) => {
                    if let Err(e) = handle_incoming(stream) {
                        log::warn!("tailnet incoming error: {}", e);
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No pending connection — sleep briefly before retrying.
                    std::thread::sleep(Duration::from_millis(100));
                }
                Err(e) => {
                    log::warn!("tailnet accept error: {}", e);
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
        log::info!("tailnet listener stopped");
        if let Ok(mut state) = transport().write() {
            state.listener_running = false;
        }
    });

    Ok(())
}

/// Stop the background listener thread.
pub fn stop_listener() {
    STOP_FLAG.store(true, Ordering::SeqCst);
}

/// Send a message to a remote node at `to_addr` (ip:port or hostname:port).
/// The message is serialised as length-prefixed JSON.
pub fn send_message(to_addr: &str, message: &MeshMessage) -> Result<(), String> {
    let mut outbound = message.clone();
    sign_message_if_needed(&mut outbound);
    let json = serde_json::to_vec(&outbound).map_err(|e| format!("serialize: {}", e))?;
    let mut stream =
        TcpStream::connect(to_addr).map_err(|e| format!("connect {}: {}", to_addr, e))?;

    if node_link::write_secure_message(&mut stream, to_addr, &json)? {
        return Ok(());
    }

    let len = json.len() as u32;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| format!("write len: {}", e))?;
    stream
        .write_all(&json)
        .map_err(|e| format!("write body: {}", e))?;

    Ok(())
}

/// Drain and return all queued inbound messages.
pub fn poll_messages() -> Vec<MeshMessage> {
    let mut state = match transport().write() {
        Ok(s) => s,
        Err(_) => return Vec::new(),
    };
    state.inbound_queue.drain(..).collect()
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn handle_incoming(mut stream: TcpStream) -> Result<(), String> {
    stream
        .set_nonblocking(false)
        .map_err(|e| format!("set blocking: {}", e))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set timeout: {}", e))?;

    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| format!("read len: {}", e))?;
    let (body, security) = node_link::secure_or_plain_body(&mut stream, len_buf)?;

    let mut msg: MeshMessage =
        serde_json::from_slice(&body).map_err(|e| format!("deserialize: {}", e))?;
    if let Some(security) = security.as_ref() {
        node_link::apply_security_context(&mut msg, security);
    }

    // Wave 4.2: Validate HMAC and timestamp
    validate_message(&msg)?;

    // Actor mailbox posting is now handled by the runtime IPC system.

    let mut state = transport().write().map_err(|e| format!("lock: {}", e))?;
    state.inbound_queue.push_back(msg);
    Ok(())
}
