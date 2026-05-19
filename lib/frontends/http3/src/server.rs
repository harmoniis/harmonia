use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};

use bytes::{Buf, Bytes};
use h3::server::RequestStream;
use http::{Method, Request, Response, StatusCode};
use rustls::server::WebPkiClientVerifier;
use tokio::sync::{mpsc, watch};

use crate::model::{
    escape_metadata, now_ms, FrontendConfig, InboundSignal, SessionHandle, VerifiedPeer,
};

pub(crate) fn route_key(identity_fingerprint: &str, session_id: &str, channel: &str) -> String {
    format!(
        "{}/{}/{}",
        harmonia_transport_auth::normalize_fingerprint(identity_fingerprint),
        session_id.trim(),
        if channel.trim().is_empty() {
            "default"
        } else {
            channel.trim()
        }
    )
}

fn parse_route(path: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = path.trim_matches('/').split('/').collect();
    if parts.len() < 3 || parts[0] != "v1" || parts[1] != "stream" {
        return None;
    }
    let session_id = parts[2].trim();
    if session_id.is_empty() {
        return None;
    }
    let channel = parts.get(3).copied().unwrap_or("default").trim();
    Some((session_id.to_string(), channel.to_string()))
}

fn metadata_sexp(peer: &VerifiedPeer, session_id: &str, path: &str) -> String {
    format!(
        "(:origin-fp \"{}\" :tls-cert-fp \"{}\" :fingerprint-valid t :trusted-origin t :transport-security \"mtls\" :channel-class \"http3-client\" :node-id \"{}\" :node-label \"{}\" :node-role \"remote-user\" :session-id \"{}\" :http3-path \"{}\" :remote t)",
        escape_metadata(&peer.identity_fingerprint),
        escape_metadata(&peer.cert_fingerprint),
        escape_metadata(&peer.identity_fingerprint),
        escape_metadata(&peer.identity_fingerprint),
        escape_metadata(session_id),
        escape_metadata(path)
    )
}

fn merge_metadata(base: &str, extra: Option<&str>) -> String {
    fn trim_parens(value: &str) -> &str {
        let trimmed = value.trim();
        if trimmed.starts_with('(') && trimmed.ends_with(')') && trimmed.len() >= 2 {
            &trimmed[1..trimmed.len() - 1]
        } else {
            trimmed
        }
    }
    match extra.map(str::trim).filter(|value| !value.is_empty()) {
        Some(extra) => format!("({} {})", trim_parens(base), trim_parens(extra)),
        None => base.to_string(),
    }
}

fn parse_payload_line(line: &str) -> Result<(String, Option<String>), String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return Err("empty frame".to_string());
    }
    let json: serde_json::Value =
        serde_json::from_str(trimmed).map_err(|e| format!("invalid NDJSON frame: {e}"))?;
    let payload = json
        .get("payload")
        .and_then(|value| value.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| json.to_string());
    let metadata = json
        .get("metadata")
        .and_then(|value| value.as_str())
        .map(str::to_string);
    Ok((payload, metadata))
}

/// Build the rustls + quinn server config from the loaded frontend config.
fn build_quinn_config(config: &FrontendConfig) -> Result<quinn::ServerConfig, String> {
    let server_cert_chain = harmonia_transport_auth::load_cert_chain(&config.server_cert)?;
    let server_key = harmonia_transport_auth::load_private_key(&config.server_key)?;
    let roots = harmonia_transport_auth::load_root_store(&config.ca_cert)?;
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| format!("client verifier init failed: {e}"))?;
    let mut tls_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(server_cert_chain, server_key)
        .map_err(|e| format!("server TLS config failed: {e}"))?;
    tls_config.alpn_protocols = vec![b"h3".to_vec()];
    let qsc = quinn::crypto::rustls::QuicServerConfig::try_from(tls_config)
        .map_err(|e| format!("quinn rustls config failed: {e}"))?;
    let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(qsc));
    let mut transport = quinn::TransportConfig::default();
    transport.max_concurrent_bidi_streams(quinn::VarInt::from_u32(config.max_concurrent_streams));
    server_config.transport = Arc::new(transport);
    Ok(server_config)
}

pub(crate) async fn run_server(
    config: FrontendConfig,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    mut shutdown_rx: watch::Receiver<bool>,
    ready_tx: std::sync::mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let server_config = match build_quinn_config(&config) {
        Ok(c) => c,
        Err(e) => {
            let _ = ready_tx.send(Err(e.clone()));
            return Err(e);
        }
    };
    let endpoint = match quinn::Endpoint::server(server_config, config.bind) {
        Ok(ep) => ep,
        Err(e) => {
            let msg = format!("quinn endpoint bind {} failed: {e}", config.bind);
            let _ = ready_tx.send(Err(msg.clone()));
            return Err(msg);
        }
    };
    let _ = ready_tx.send(Ok(()));

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                match changed {
                    Ok(()) if *shutdown_rx.borrow() => break,
                    Ok(()) => continue,
                    Err(_) => break,
                }
            }
            incoming = endpoint.accept() => {
                let Some(connecting) = incoming else { break; };
                let trusted = config.trusted_fingerprints.clone();
                let cfg = config.clone();
                let inbound = inbound.clone();
                let sessions = sessions.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_connection(connecting, trusted, cfg, inbound, sessions).await {
                        eprintln!("[WARN] [http3] connection failed: {e}");
                    }
                });
            }
        }
    }
    endpoint.wait_idle().await;
    Ok(())
}

async fn handle_connection(
    connecting: quinn::Incoming,
    trusted: Arc<std::collections::HashSet<String>>,
    config: FrontendConfig,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
) -> Result<(), String> {
    let conn = connecting
        .await
        .map_err(|e| format!("quinn handshake failed: {e}"))?;

    // Pull the peer cert out of the handshake to verify it against our
    // trusted-fingerprints set. quinn surfaces the rustls handshake data on
    // the connection.
    let peer_cert_der = match conn
        .peer_identity()
        .and_then(|id| id.downcast::<Vec<rustls::pki_types::CertificateDer<'static>>>().ok())
    {
        Some(certs) => certs
            .first()
            .cloned()
            .ok_or_else(|| "client did not present a certificate".to_string())?,
        None => return Err("client identity missing from handshake".to_string()),
    };
    let verified =
        harmonia_transport_auth::verify_client_certificate_der(peer_cert_der.as_ref(), &trusted)?;
    let peer = VerifiedPeer {
        identity_fingerprint: verified.identity_fingerprint,
        cert_fingerprint: verified.cert_fingerprint,
    };

    let mut h3_conn: h3::server::Connection<h3_quinn::Connection, Bytes> =
        h3::server::Connection::new(h3_quinn::Connection::new(conn))
            .await
            .map_err(|e| format!("h3 connection setup failed: {e}"))?;

    while let Ok(Some(resolver)) = h3_conn.accept().await {
        let (req, stream) = match resolver.resolve_request().await {
            Ok(pair) => pair,
            Err(e) => {
                eprintln!("[WARN] [http3] resolve_request: {e}");
                continue;
            }
        };
        let peer = peer.clone();
        let inbound = inbound.clone();
        let sessions = sessions.clone();
        let cfg = config.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_request(req, stream, peer, inbound, sessions, cfg).await {
                eprintln!("[WARN] [http3] request handler failed: {e}");
            }
        });
    }
    Ok(())
}

async fn handle_request<S>(
    request: Request<()>,
    mut stream: RequestStream<S, Bytes>,
    peer: VerifiedPeer,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    config: FrontendConfig,
) -> Result<(), String>
where
    S: h3::quic::BidiStream<Bytes> + Send + 'static,
    <S as h3::quic::BidiStream<Bytes>>::SendStream: Send + 'static,
    <S as h3::quic::BidiStream<Bytes>>::RecvStream: Send + 'static,
{
    if request.method() != Method::POST {
        let response = Response::builder()
            .status(StatusCode::METHOD_NOT_ALLOWED)
            .body(())
            .map_err(|e| format!("response build: {e}"))?;
        stream
            .send_response(response)
            .await
            .map_err(|e| format!("send_response: {e}"))?;
        stream
            .finish()
            .await
            .map_err(|e| format!("finish: {e}"))?;
        return Ok(());
    }

    let path = request.uri().path().to_string();
    let Some((session_id, channel)) = parse_route(&path) else {
        let response = Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(())
            .map_err(|e| format!("response build: {e}"))?;
        stream
            .send_response(response)
            .await
            .map_err(|e| format!("send_response: {e}"))?;
        stream
            .finish()
            .await
            .map_err(|e| format!("finish: {e}"))?;
        return Ok(());
    };

    let sub_channel = route_key(&peer.identity_fingerprint, &session_id, &channel);
    let metadata = metadata_sexp(&peer, &session_id, &path);
    let last_activity_ms = Arc::new(AtomicU64::new(now_ms()));
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Bytes>(128);

    if let Ok(mut guard) = sessions.write() {
        guard.insert(
            sub_channel.clone(),
            SessionHandle {
                outbound: outbound_tx.clone(),
                last_activity_ms: last_activity_ms.clone(),
            },
        );
    }

    let response = Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/x-ndjson")
        .body(())
        .map_err(|e| format!("response build: {e}"))?;
    stream
        .send_response(response)
        .await
        .map_err(|e| format!("send_response: {e}"))?;

    let (mut send_stream, mut recv_stream) = stream.split();

    // Pump outbound queue → send_stream until the channel closes.
    let outbound_task = tokio::spawn(async move {
        while let Some(chunk) = outbound_rx.recv().await {
            if let Err(e) = send_stream.send_data(chunk).await {
                eprintln!("[WARN] [http3] outbound send failed: {e}");
                break;
            }
        }
        let _ = send_stream.finish().await;
    });

    // Pump inbound recv_stream → buffer → NDJSON lines → inbound queue.
    let max_frame_bytes = config.max_frame_bytes;
    let sub_channel_for_inbound = sub_channel.clone();
    let inbound_clone = inbound.clone();
    let last_activity_for_inbound = last_activity_ms.clone();
    let inbound_task = tokio::spawn(async move {
        let mut buffer: Vec<u8> = Vec::new();
        loop {
            let mut chunk = match recv_stream.recv_data().await {
                Ok(Some(b)) => b,
                Ok(None) => break,
                Err(e) => {
                    eprintln!("[WARN] [http3] recv_data: {e}");
                    break;
                }
            };
            // h3 returns `impl Buf`; drain it into the rolling buffer.
            while chunk.has_remaining() {
                let slice = chunk.chunk();
                let n = slice.len();
                buffer.extend_from_slice(slice);
                chunk.advance(n);
            }
            if buffer.len() > max_frame_bytes {
                break;
            }
            while let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
                let line = String::from_utf8_lossy(&buffer[..index]).to_string();
                buffer.drain(..=index);
                if let Ok((payload, extra_metadata)) = parse_payload_line(&line) {
                    last_activity_for_inbound.store(now_ms(), Ordering::Relaxed);
                    if let Ok(mut q) = inbound_clone.lock() {
                        q.push_back(InboundSignal {
                            sub_channel: sub_channel_for_inbound.clone(),
                            payload,
                            metadata: merge_metadata(&metadata, extra_metadata.as_deref()),
                        });
                    }
                }
            }
        }
        // Trailing partial line, if any.
        if !buffer.is_empty() {
            if let Ok(line) = String::from_utf8(buffer) {
                if let Ok((payload, extra_metadata)) = parse_payload_line(&line) {
                    last_activity_for_inbound.store(now_ms(), Ordering::Relaxed);
                    if let Ok(mut q) = inbound_clone.lock() {
                        q.push_back(InboundSignal {
                            sub_channel: sub_channel_for_inbound,
                            payload,
                            metadata: merge_metadata(&metadata, extra_metadata.as_deref()),
                        });
                    }
                }
            }
        }
    });

    // Idle-timeout / cleanup: when the inbound task ends, drop the session
    // entry so the outbound task's channel closes and `outbound_task` exits.
    let sessions_for_cleanup = sessions.clone();
    let sub_channel_for_cleanup = sub_channel.clone();
    let session_idle_timeout_ms = config.session_idle_timeout_ms;
    tokio::spawn(async move {
        let _ = inbound_task.await;
        // Wait one idle timeout for any in-flight outbound writes.
        tokio::time::sleep(std::time::Duration::from_millis(session_idle_timeout_ms.min(1000))).await;
        if let Ok(mut g) = sessions_for_cleanup.write() {
            g.remove(&sub_channel_for_cleanup);
        }
        let _ = outbound_task.await;
    });

    Ok(())
}

pub(crate) fn enqueue_outbound(
    outbound: &mpsc::Sender<Bytes>,
    bytes: Bytes,
) -> Result<(), String> {
    outbound
        .try_send(bytes)
        .map_err(|e| format!("outbound channel full or closed: {e}"))
}
