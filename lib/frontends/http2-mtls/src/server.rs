use bytes::Bytes;
use futures_util::stream;
use http::header::{HeaderValue, CONTENT_TYPE};
use http::{Method, Request, Response, StatusCode};
use http_body_util::{BodyExt, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::server::WebPkiClientVerifier;
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio_rustls::TlsAcceptor;

use crate::model::{
    boxed_full, escape_metadata, now_ms, FrontendConfig, HttpBody, InboundSignal, SessionHandle,
    VerifiedPeer,
};
use crate::session::session_cleanup_loop;

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

fn response_with(status: StatusCode, body: impl Into<Bytes>) -> Response<HttpBody> {
    let mut response = Response::new(boxed_full(body.into()));
    *response.status_mut() = status;
    response
}

fn metadata_sexp(peer: &VerifiedPeer, session_id: &str, path: &str) -> String {
    format!(
        "(:origin-fp \"{}\" :tls-cert-fp \"{}\" :fingerprint-valid t :trusted-origin t :transport-security \"mtls\" :channel-class \"http2-client\" :node-id \"{}\" :node-label \"{}\" :node-role \"remote-user\" :session-id \"{}\" :http2-path \"{}\" :remote t)",
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

fn stream_body(receiver: mpsc::Receiver<Bytes>) -> HttpBody {
    StreamBody::new(stream::unfold(receiver, |mut receiver| async move {
        receiver
            .recv()
            .await
            .map(|chunk| (Ok::<Frame<Bytes>, Infallible>(Frame::data(chunk)), receiver))
    }))
    .boxed()
}

async fn read_request_stream(
    mut body: Incoming,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sub_channel: String,
    metadata: String,
    last_activity_ms: Arc<AtomicU64>,
    max_frame_bytes: usize,
) {
    let mut buffer = Vec::new();
    while let Some(frame_result) = body.frame().await {
        let Ok(frame) = frame_result else {
            break;
        };
        let Some(data) = frame.data_ref() else {
            continue;
        };
        buffer.extend_from_slice(data);
        if buffer.len() > max_frame_bytes {
            break;
        }
        while let Some(index) = buffer.iter().position(|byte| *byte == b'\n') {
            let line = String::from_utf8_lossy(&buffer[..index]).to_string();
            buffer.drain(..=index);
            if let Ok((payload, extra_metadata)) = parse_payload_line(&line) {
                last_activity_ms.store(now_ms(), Ordering::Relaxed);
                if let Ok(mut queue) = inbound.lock() {
                    queue.push_back(InboundSignal {
                        sub_channel: sub_channel.clone(),
                        payload,
                        metadata: merge_metadata(&metadata, extra_metadata.as_deref()),
                    });
                }
            }
        }
    }

    if !buffer.is_empty() {
        if let Ok(line) = String::from_utf8(buffer) {
            if let Ok((payload, extra_metadata)) = parse_payload_line(&line) {
                last_activity_ms.store(now_ms(), Ordering::Relaxed);
                if let Ok(mut queue) = inbound.lock() {
                    queue.push_back(InboundSignal {
                        sub_channel,
                        payload,
                        metadata: merge_metadata(&metadata, extra_metadata.as_deref()),
                    });
                }
            }
        }
    }
}

async fn handle_stream_request(
    request: Request<Incoming>,
    peer: VerifiedPeer,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    session_idle_timeout_ms: u64,
    max_frame_bytes: usize,
) -> Result<Response<HttpBody>, Infallible> {
    if request.method() != Method::POST {
        return Ok(response_with(
            StatusCode::METHOD_NOT_ALLOWED,
            Bytes::from_static(b"method not allowed"),
        ));
    }

    let path = request.uri().path().to_string();
    let Some((session_id, channel)) = parse_route(&path) else {
        return Ok(response_with(
            StatusCode::NOT_FOUND,
            Bytes::from_static(b"unknown route"),
        ));
    };

    let sub_channel = route_key(&peer.identity_fingerprint, &session_id, &channel);
    let metadata = metadata_sexp(&peer, &session_id, &path);
    let last_activity_ms = Arc::new(AtomicU64::new(now_ms()));
    let (outbound_tx, outbound_rx) = mpsc::channel::<Bytes>(128);
    let body = request.into_body();

    if let Ok(mut guard) = sessions.write() {
        guard.insert(
            sub_channel.clone(),
            SessionHandle {
                outbound: outbound_tx.clone(),
                last_activity_ms: last_activity_ms.clone(),
            },
        );
    }

    tokio::spawn(read_request_stream(
        body,
        inbound,
        sub_channel.clone(),
        metadata,
        last_activity_ms.clone(),
        max_frame_bytes,
    ));
    tokio::spawn(session_cleanup_loop(
        sessions.clone(),
        sub_channel.clone(),
        outbound_tx.clone(),
        last_activity_ms,
        session_idle_timeout_ms,
    ));

    let mut response = Response::new(stream_body(outbound_rx));
    response.headers_mut().insert(
        CONTENT_TYPE,
        HeaderValue::from_static("application/x-ndjson"),
    );
    Ok(response)
}

async fn handle_connection(
    socket: tokio::net::TcpStream,
    acceptor: TlsAcceptor,
    config: FrontendConfig,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
) -> Result<(), String> {
    let tls = acceptor
        .accept(socket)
        .await
        .map_err(|e| format!("tls accept failed: {e}"))?;
    let peer_cert = tls
        .get_ref()
        .1
        .peer_certificates()
        .and_then(|certs| certs.first())
        .cloned()
        .ok_or_else(|| "client certificate was not presented".to_string())?;
    let verified = harmonia_transport_auth::verify_client_certificate_der(
        peer_cert.as_ref(),
        &config.trusted_fingerprints,
    )?;
    let peer = VerifiedPeer {
        identity_fingerprint: verified.identity_fingerprint,
        cert_fingerprint: verified.cert_fingerprint,
    };

    let io = TokioIo::new(tls);
    let service = service_fn(move |request| {
        handle_stream_request(
            request,
            peer.clone(),
            inbound.clone(),
            sessions.clone(),
            config.session_idle_timeout_ms,
            config.max_frame_bytes,
        )
    });
    let mut builder = http2::Builder::new(TokioExecutor::new());
    builder.max_concurrent_streams(config.max_concurrent_streams);
    builder
        .serve_connection(io, service)
        .await
        .map_err(|e| format!("http2 connection failed: {e}"))?;
    Ok(())
}

pub(crate) async fn run_server(
    config: FrontendConfig,
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    mut shutdown_rx: watch::Receiver<bool>,
    ready_tx: std::sync::mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    let server_cert_chain = match harmonia_transport_auth::load_cert_chain(&config.server_cert) {
        Ok(chain) => chain,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    let server_key = match harmonia_transport_auth::load_private_key(&config.server_key) {
        Ok(key) => key,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    let roots = match harmonia_transport_auth::load_root_store(&config.ca_cert) {
        Ok(roots) => roots,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    let verifier = WebPkiClientVerifier::builder(Arc::new(roots))
        .build()
        .map_err(|e| format!("client verifier init failed: {e}"));
    let verifier = match verifier {
        Ok(verifier) => verifier,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    let server_config = rustls::ServerConfig::builder()
        .with_client_cert_verifier(verifier)
        .with_single_cert(server_cert_chain, server_key)
        .map_err(|e| format!("server TLS config failed: {e}"));
    let mut server_config = match server_config {
        Ok(config) => config,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    server_config.alpn_protocols = vec![b"h2".to_vec()];

    let listener = TcpListener::bind(config.bind)
        .await
        .map_err(|e| format!("bind {} failed: {e}", config.bind));
    let listener = match listener {
        Ok(listener) => listener,
        Err(error) => {
            let _ = ready_tx.send(Err(error.clone()));
            return Err(error);
        }
    };
    let _ = ready_tx.send(Ok(()));
    let acceptor = TlsAcceptor::from(Arc::new(server_config));

    loop {
        tokio::select! {
            changed = shutdown_rx.changed() => {
                match changed {
                    Ok(()) if *shutdown_rx.borrow() => break,
                    Ok(()) => continue,
                    Err(_) => break,
                }
            }
            accepted = listener.accept() => {
                let (socket, _) = accepted.map_err(|e| format!("accept failed: {e}"))?;
                let acceptor = acceptor.clone();
                let inbound = inbound.clone();
                let sessions = sessions.clone();
                let config = config.clone();
                tokio::spawn(async move {
                    let _ = handle_connection(socket, acceptor, config, inbound, sessions).await;
                });
            }
        }
    }

    Ok(())
}
