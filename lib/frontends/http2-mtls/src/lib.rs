use bytes::Bytes;
use futures_util::stream;
use http::header::{HeaderValue, CONTENT_TYPE};
use http::{Method, Request, Response, StatusCode};
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::{Frame, Incoming};
use hyper::server::conn::http2;
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use rustls::server::WebPkiClientVerifier;
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::ffi::{CStr, CString};
use std::net::SocketAddr;
use std::os::raw::c_char;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{mpsc, watch};
use tokio_rustls::TlsAcceptor;

const VERSION: &[u8] = b"harmonia-http2-mtls/0.1.0\0";
const COMPONENT: &str = "http2-frontend";
const DEFAULT_BIND: &str = "127.0.0.1:9443";
const DEFAULT_MAX_STREAMS: u32 = 64;
const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_MAX_FRAME_BYTES: usize = 64 * 1024;
const LINEAGE_SYMBOL: &str = "http2_tls_master_seed";

type HttpBody = http_body_util::combinators::BoxBody<Bytes, Infallible>;

#[derive(Clone)]
struct InboundSignal {
    sub_channel: String,
    payload: String,
    metadata: String,
}

#[derive(Clone)]
struct SessionHandle {
    outbound: mpsc::Sender<Bytes>,
    last_activity_ms: Arc<AtomicU64>,
}

struct FrontendState {
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    shutdown: watch::Sender<bool>,
    server_thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
struct FrontendConfig {
    bind: SocketAddr,
    ca_cert: std::path::PathBuf,
    server_cert: std::path::PathBuf,
    server_key: std::path::PathBuf,
    trusted_fingerprints: Arc<std::collections::HashSet<String>>,
    max_concurrent_streams: u32,
    session_idle_timeout_ms: u64,
    max_frame_bytes: usize,
}

#[derive(Clone)]
struct VerifiedPeer {
    identity_fingerprint: String,
    cert_fingerprint: String,
}

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
static STATE: OnceLock<Mutex<Option<FrontendState>>> = OnceLock::new();

fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn state_slot() -> &'static Mutex<Option<FrontendState>> {
    STATE.get_or_init(|| Mutex::new(None))
}

fn set_error(message: impl Into<String>) {
    if let Ok(mut guard) = last_error().write() {
        *guard = message.into();
    }
}

fn clear_error() {
    if let Ok(mut guard) = last_error().write() {
        guard.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let raw = unsafe { CStr::from_ptr(ptr) };
    Ok(raw.to_string_lossy().into_owned())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn boxed_full(payload: impl Into<Bytes>) -> HttpBody {
    Full::new(payload.into())
        .map_err(|never| match never {})
        .boxed()
}

fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn route_key(identity_fingerprint: &str, session_id: &str, channel: &str) -> String {
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

fn load_config() -> Result<FrontendConfig, String> {
    harmonia_config_store::init().map_err(|e| format!("config-store init failed: {e}"))?;
    let _ = harmonia_transport_auth::record_tls_lineage_seed(COMPONENT, "tls", LINEAGE_SYMBOL);

    let bind = harmonia_config_store::get_own_or(COMPONENT, "bind", DEFAULT_BIND)
        .unwrap_or_else(|_| DEFAULT_BIND.to_string())
        .parse::<SocketAddr>()
        .map_err(|e| format!("invalid http2-frontend/bind: {e}"))?;
    let ca_cert = harmonia_transport_auth::required_config_path(COMPONENT, "ca-cert")?;
    let server_cert = harmonia_transport_auth::required_config_path(COMPONENT, "server-cert")?;
    let server_key = harmonia_transport_auth::required_config_path(COMPONENT, "server-key")?;
    let trusted_fingerprints = harmonia_transport_auth::load_trusted_fingerprints(
        COMPONENT,
        harmonia_transport_auth::DEFAULT_TRUST_SCOPE_KEY,
    );
    if trusted_fingerprints.is_empty() {
        return Err(
            "http2-frontend/trusted-client-fingerprints-json must contain at least one trusted client identity"
                .to_string(),
        );
    }
    let max_concurrent_streams =
        harmonia_config_store::get_own(COMPONENT, "max-concurrent-streams")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(DEFAULT_MAX_STREAMS);
    let session_idle_timeout_ms =
        harmonia_config_store::get_own(COMPONENT, "session-idle-timeout-ms")
            .ok()
            .flatten()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_IDLE_TIMEOUT_MS);
    let max_frame_bytes = harmonia_config_store::get_own(COMPONENT, "max-frame-bytes")
        .ok()
        .flatten()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(DEFAULT_MAX_FRAME_BYTES);

    Ok(FrontendConfig {
        bind,
        ca_cert,
        server_cert,
        server_key,
        trusted_fingerprints: Arc::new(trusted_fingerprints),
        max_concurrent_streams,
        session_idle_timeout_ms,
        max_frame_bytes,
    })
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

async fn session_cleanup_loop(
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    route: String,
    sender: mpsc::Sender<Bytes>,
    last_activity_ms: Arc<AtomicU64>,
    idle_timeout_ms: u64,
) {
    loop {
        tokio::select! {
            _ = sender.closed() => break,
            _ = tokio::time::sleep(Duration::from_millis(idle_timeout_ms.max(250))) => {
                if now_ms().saturating_sub(last_activity_ms.load(Ordering::Relaxed)) > idle_timeout_ms {
                    break;
                }
            }
        }
    }

    if let Ok(mut guard) = sessions.write() {
        guard.remove(&route);
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

async fn run_server(
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

fn take_state() -> Option<FrontendState> {
    state_slot().lock().ok().and_then(|mut guard| guard.take())
}

fn enqueue_outbound(outbound: &mpsc::Sender<Bytes>, payload: Bytes) -> Result<(), String> {
    match outbound.try_send(payload) {
        Ok(()) => Ok(()),
        Err(tokio::sync::mpsc::error::TrySendError::Closed(_)) => Err("stream closed".to_string()),
        Err(tokio::sync::mpsc::error::TrySendError::Full(payload)) => {
            if let Ok(handle) = tokio::runtime::Handle::try_current() {
                let outbound = outbound.clone();
                handle.spawn(async move {
                    let _ = outbound.send(payload).await;
                });
                Ok(())
            } else {
                outbound
                    .blocking_send(payload)
                    .map_err(|_| "stream closed".to_string())
            }
        }
    }
}

fn start_server() -> Result<FrontendState, String> {
    let config = load_config()?;
    let inbound = Arc::new(Mutex::new(VecDeque::new()));
    let sessions = Arc::new(RwLock::new(HashMap::new()));
    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let inbound_clone = inbound.clone();
    let sessions_clone = sessions.clone();
    let server_thread = std::thread::Builder::new()
        .name("harmonia-http2-mtls".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("http2 frontend runtime");
            let result = runtime.block_on(run_server(
                config,
                inbound_clone,
                sessions_clone,
                shutdown_rx,
                ready_tx,
            ));
            if let Err(error) = result {
                set_error(error);
            }
        })
        .map_err(|e| format!("spawn http2 server thread failed: {e}"))?;

    match ready_rx.recv_timeout(Duration::from_secs(3)) {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            let _ = shutdown_tx.send(true);
            let _ = server_thread.join();
            return Err(error);
        }
        Err(_) => {
            let _ = shutdown_tx.send(true);
            let _ = server_thread.join();
            return Err("http2 frontend listener did not become ready".to_string());
        }
    }

    Ok(FrontendState {
        inbound,
        sessions,
        shutdown: shutdown_tx,
        server_thread: Some(server_thread),
    })
}

fn shutdown_state(mut state: FrontendState) {
    let _ = state.shutdown.send(true);
    if let Some(handle) = state.server_thread.take() {
        let _ = handle.join();
    }
}

pub fn harmonia_frontend_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

pub fn harmonia_frontend_healthcheck() -> i32 {
    1
}

pub fn harmonia_frontend_init(config: *const c_char) -> i32 {
    let _ = cstr_to_string(config);
    if let Some(previous) = take_state() {
        shutdown_state(previous);
    }
    match start_server() {
        Ok(state) => {
            if let Ok(mut guard) = state_slot().lock() {
                *guard = Some(state);
            }
            clear_error();
            0
        }
        Err(error) => {
            set_error(error);
            -1
        }
    }
}

pub fn harmonia_frontend_poll(buf: *mut c_char, buf_len: usize) -> i32 {
    if buf.is_null() || buf_len == 0 {
        set_error("poll: null buffer or zero length");
        return -1;
    }
    let Some(inbound) = state_slot()
        .lock()
        .ok()
        .and_then(|guard| guard.as_ref().map(|state| state.inbound.clone()))
    else {
        return 0;
    };

    let mut output = String::new();
    if let Ok(mut queue) = inbound.lock() {
        while let Some(signal) = queue.pop_front() {
            let line = format!(
                "{}\t{}\t{}\n",
                signal.sub_channel, signal.payload, signal.metadata
            );
            if output.len() + line.len() >= buf_len.saturating_sub(1) {
                queue.push_front(signal);
                break;
            }
            output.push_str(&line);
        }
    }

    if output.is_empty() {
        return 0;
    }

    let bytes = output.as_bytes();
    let count = bytes.len().min(buf_len.saturating_sub(1));
    unsafe {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), buf as *mut u8, count);
        *((buf as *mut u8).add(count)) = 0;
    }
    clear_error();
    count as i32
}

pub fn harmonia_frontend_send(channel: *const c_char, payload: *const c_char) -> i32 {
    let route = match cstr_to_string(channel) {
        Ok(route) => route,
        Err(error) => {
            set_error(error);
            return -1;
        }
    };
    let payload = match cstr_to_string(payload) {
        Ok(payload) => payload,
        Err(error) => {
            set_error(error);
            return -1;
        }
    };

    let Some((outbound, last_activity_ms)) = state_slot().lock().ok().and_then(|guard| {
        guard.as_ref().and_then(|state| {
            state.sessions.read().ok().and_then(|sessions| {
                sessions
                    .get(&route)
                    .map(|handle| (handle.outbound.clone(), handle.last_activity_ms.clone()))
            })
        })
    }) else {
        set_error(format!("no active HTTP/2 stream for route {route}"));
        return -1;
    };

    let json = match serde_json::to_string(&serde_json::json!({ "payload": payload })) {
        Ok(json) => json + "\n",
        Err(error) => {
            set_error(format!("serialize outbound payload failed: {error}"));
            return -1;
        }
    };
    if let Err(error) = enqueue_outbound(&outbound, Bytes::from(json)) {
        set_error(format!("failed sending to HTTP/2 stream {route}: {error}"));
        return -1;
    }
    last_activity_ms.store(now_ms(), Ordering::Relaxed);
    clear_error();
    0
}

pub fn harmonia_frontend_last_error() -> *const c_char {
    let message = last_error()
        .read()
        .map(|guard| guard.clone())
        .unwrap_or_else(|_| "http2 frontend lock poisoned".to_string());
    CString::new(message)
        .map(|value| value.into_raw() as *const c_char)
        .unwrap_or(std::ptr::null())
}

pub fn harmonia_frontend_shutdown() -> i32 {
    if let Some(state) = take_state() {
        shutdown_state(state);
    }
    clear_error();
    0
}

pub fn harmonia_frontend_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

pub fn harmonia_frontend_list_channels() -> *const c_char {
    let channels = state_slot()
        .lock()
        .ok()
        .and_then(|guard| {
            guard.as_ref().map(|state| {
                state
                    .sessions
                    .read()
                    .map(|sessions| sessions.keys().cloned().collect::<Vec<_>>())
                    .unwrap_or_default()
            })
        })
        .unwrap_or_default();
    let sexp = if channels.is_empty() {
        "nil".to_string()
    } else {
        format!(
            "({})",
            channels
                .iter()
                .map(|channel| format!("\"{}\"", channel))
                .collect::<Vec<_>>()
                .join(" ")
        )
    };
    CString::new(sexp)
        .map(|value| value.into_raw() as *const c_char)
        .unwrap_or(std::ptr::null())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bytes::Bytes;
    use http_body_util::{BodyExt, Full};
    use hyper::Request;
    use hyper_util::rt::TokioIo;
    use rustls_pki_types::ServerName;
    use std::ffi::CString;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex, OnceLock};
    use tokio::net::TcpStream;
    use tokio_rustls::TlsConnector;

    const CA_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDJzCCAg+gAwIBAgIUBRxy9iDE6Q7JHSNQznvI6Fp7woIwDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQaGFybW9uaWEtdGVzdC1jYTAeFw0yNjAzMTcxNTI4NDda
Fw0yNzAzMTcxNTI4NDdaMBsxGTAXBgNVBAMMEGhhcm1vbmlhLXRlc3QtY2EwggEi
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQDhUdzyV59SNOZbUGbjCmSQHYKF
TqmNAXv5xfLLKTPlF/8d/pNATJ/+O+2dPkFO17O84fhi0wChrlCUBBwA5aweUMV7
kGUquHu6Z/ZBfNqJbg5NPaxLMDw0lI0J6HzQNKkH5ajqVfLFsI1kgUXh5ziB8/kx
T+LVxmsiU6/g4/czoGSnjNeAa9QqtqmBaYhPHqoOI5fDabIsBUgMbf0ogEoKDmd2
Ip1SLCi0EDPAPyev8E4F8iLCqTSWGXzcQF8xsrFwmeOkKoIDmoHIq88ThgDB+7zG
USRwGoT4Qspr6QjXHeJ2Ilhs/f2yda6GEU+1Ly5BTf4EDY4xy0dYktHzysxvAgMB
AAGjYzBhMA8GA1UdEwEB/wQFMAMBAf8wDgYDVR0PAQH/BAQDAgEGMB0GA1UdDgQW
BBSL+da7Qj6+Np3yJYsChFf4ZjintTAfBgNVHSMEGDAWgBSL+da7Qj6+Np3yJYsC
hFf4ZjintTANBgkqhkiG9w0BAQsFAAOCAQEAKMeFTaCASFMmyY34e8BJ34NiZ90d
qmG2jP49mqrQ45yBtjVe+tpB9utwfkCTuUy3UvcMZ3vDXQbbooMG91UPqzPJu1vC
7YJoKfJQbI9iiV1Y02ZEQYdz5tattlK3NBQhk0lm+T+4qujM3Cfbh350F4DwNNK+
WNTKLK6aHdLf/PWxOgMruUlOLwAtfOZB30EISd0zm5wYb0Zr+7B7gq7OaoPsF+eR
bPoH4sZKHTKpdZtDLjK+4fA9svY/tjSyA5R2vFYa/ZCy5OyutsmrOpU5wA7ELEIt
Lo29zUialeHObCra6uopmg7LKrzikDkIAT9SEwtohrCEm3GYZlw6GUJDTw==
-----END CERTIFICATE-----"#;

    const SERVER_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDXzCCAkegAwIBAgIUC/O+UKWoQTUiXw1T8edHYc/unmEwDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQaGFybW9uaWEtdGVzdC1jYTAeFw0yNjAzMTcxNTI4NDda
Fw0yNzAzMTcxNTI4NDdaMCAxHjAcBgNVBAMMFWhhcm1vbmlhLWh0dHAyLXNlcnZl
cjCCASIwDQYJKoZIhvcNAQEBBQADggEPADCCAQoCggEBAPG3glSrTgZ814SDaGZD
TVEdm7bxGl8HNIYIw5LeOs7CDiyQM7wLMTfiRaryJBWgG01k6+eL9a/FZ+mw+XgN
qSYfJ2+L9zuL9saVNk2mQCKVCdWYIY9ukcq05bsYBM6IkW4M2Zeaqe0IwfLdQXBE
8soOICwaz/uSjEddMLKpe4LrLThTXSQomwZjE0Y1VJYsAWgiM4eBLhOzS6iOfJTi
xVoREKqUI4n6PfUGenGT/Fx+ng9fbp6YYsK94z54V3ctom89exig1ffWZPgjrHYZ
vbtCvp+Cc5zWJB+fjJrei8sw14/QDue8MKK4s5gMGkGp8CTD8+8u0doz37QTI9OG
Mv0CAwEAAaOBlTCBkjAgBgNVHREEGTAXghVoYXJtb25pYS1odHRwMi1zZXJ2ZXIw
CQYDVR0TBAIwADAOBgNVHQ8BAf8EBAMCBaAwEwYDVR0lBAwwCgYIKwYBBQUHAwEw
HQYDVR0OBBYEFMl9dVxKMszZ/RIjhfdGpspi2s/DMB8GA1UdIwQYMBaAFIv51rtC
Pr42nfIliwKEV/hmOKe1MA0GCSqGSIb3DQEBCwUAA4IBAQDKNir8aGhzq2QHLaZW
T9y4BbHouzZ8zKJgu0zDTP4PIp7VasAWUqkwpfEosht3cCnBsHMhClRI3+82mgWB
a0v9z5b0ymd076f9EGqSBAQruZl80fLAHJFiE7UY/qA5kuLodCee5AI8pm3a2eUb
pghCI3WxFVCezDoxTys3mgRt/m7kkbik++F8KRzGksJDz9/W8iQslJSmt9uzfZu8
VHPiEdnsyudPg24dbaFDBA7+KDCIgRkVKCs0VGZSqogE/YKTxRNCny54D/HuBINS
ukO038AKzlnfZL0X8R6/RKkVubJbd3c7udT+m4b29xv3fw+tXnrN0KvvDsnLZu+l
SlRm
-----END CERTIFICATE-----"#;

    const SERVER_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvQIBADANBgkqhkiG9w0BAQEFAASCBKcwggSjAgEAAoIBAQDxt4JUq04GfNeE
g2hmQ01RHZu28RpfBzSGCMOS3jrOwg4skDO8CzE34kWq8iQVoBtNZOvni/WvxWfp
sPl4DakmHydvi/c7i/bGlTZNpkAilQnVmCGPbpHKtOW7GATOiJFuDNmXmqntCMHy
3UFwRPLKDiAsGs/7koxHXTCyqXuC6y04U10kKJsGYxNGNVSWLAFoIjOHgS4Ts0uo
jnyU4sVaERCqlCOJ+j31Bnpxk/xcfp4PX26emGLCveM+eFd3LaJvPXsYoNX31mT4
I6x2Gb27Qr6fgnOc1iQfn4ya3ovLMNeP0A7nvDCiuLOYDBpBqfAkw/PvLtHaM9+0
EyPThjL9AgMBAAECggEAXENvKJFwyXIqs4aTOYGUCBPUpZpXNhGif0zmFe/ko5oX
3fO3A56EDXA9pnghxO1lrn+IukvGnm6r8NwgBS61s3rtyxqyZpTQv9EhtrbwQSMB
a3nTyZNra+Pr0qPi5dDkLg0Sm1cqaHNQ0LqaqVdwEyccKamcXMr955mPJoshvYD4
So3hk31k/W3IE/xcm4KqH7km0RRSr/4IEnfiGh71UrSI6ryo5UqL/ZeIVWe/yRRA
+UyrNYvaf5qwYkloFlHPw2FVQ2cIiQ8pyMC9RDoWtzgcs0U+tEmHcL2Pyo8WTIAJ
GRm0gPg9QWwpd9yBgBAHDVI8UpyjMQpMF0gDKWIEcwKBgQD7RmKWEibfwHEtU9ti
LExsZdI4UTRjHEhzIJPOz+5VqLgXj1p/K9MGRVxtBBGk8sYwejOvLmaQCgdtG7ax
N2J9QokvvExpMid3WgBLFFgiJHG0OkMRpj9vaLqlnTgvzha8Da6mXMoI2yqh725y
l0H78cCRDbU6NlngfOcPN9M39wKBgQD2Qxy0SaxLy3H+KXHh6xC7ocp3roNm6faG
OhyeNDRLfVIzxzn+Z2ucfHxLHevRWpWeJA6NFNPxuY4a7GcMCoIHrzPuot9Y7u2T
OtXv4CikC0mdRnXPhPEKipUYRl3HUYApXQE7kbU3fysEv6g86u3RtVIOVkRX8acs
3fbgVfR3qwKBgD4FMXA5Kr8vkL/PYuboaDSZLToZUQTlhjxkXhc922XpLwchqwSY
nI1/sUB3MKO2CJUOlJM4sLf8wbh8jqtPMFAajCHsKDAO4Q7keA4QB3Dl7eq+Nq+0
iRPGlcsq8yNZiuL/vYvyeyuUbQFrR6ehDfhRw2YKLCEiKSzvp1hqPwghAoGAPMKu
UGVlF4Zo59b9/EntZP40YHc0gK31X4TzDq2+wWl4YMIlMvn9eSzV1grZ5lu9Url+
xZx/9sJbp5Twj+3/yzmVTKnvBZheEdeQdZEPNfp6/U0nQD6C4qDyzHyAIu+e+ZWy
+imnVrwPtyo6rl0gtH9ScasjTbeYEd/qS8upd+UCgYEA0y/8P4fMFKIbu/Bl7rp3
ezSC8DdtpFkfFn/T8e/PnDm8R2fGbhd73WDHO3oBRSQpwsd6nh8o9zik49+FiQv9
J7GrFcvMzyLEN+oxwv7sUmx9gzPXHLj56KRQdHpwoEWZ77vE6IYn0nMlYEk1kW4W
sWEAkMQW9HBMk1N8aEsfqGQ=
-----END PRIVATE KEY-----"#;

    const CLIENT_CERT: &str = r#"-----BEGIN CERTIFICATE-----
MIIDNjCCAh6gAwIBAgIUC/O+UKWoQTUiXw1T8edHYc/unmIwDQYJKoZIhvcNAQEL
BQAwGzEZMBcGA1UEAwwQaGFybW9uaWEtdGVzdC1jYTAeFw0yNjAzMTcxNTI4NDda
Fw0yNzAzMTcxNTI4NDdaMBsxGTAXBgNVBAMMEEFCQ0RFRjEyMzQ1Njc4OTAwggEi
MA0GCSqGSIb3DQEBAQUAA4IBDwAwggEKAoIBAQC8s9xcSS8VS6taeILzzmnL0Y66
JvlCHdxZCQRfboQ8pwzg7knn+R1hEVElAaV1md6wHB0chMmYEU6ao4stVXvVkHAT
94jpfHUAdi6TXH+xoiF+q6ICE/PSL7J4x/BAxxi09/XbVwYdnWJYWe4+FZrKg8hH
VIImTuOZoMEn7hCG3JToZF2nqjhY++9FcakLibMFaNq/43At+n+mjy+uqZS5cPsz
WAdO/W+3z5qayZwXgUlr+zOgnvOHfLgZIdiZr3dcOwR2l4IN0ZVhxNmYQj/fE/Rd
hNM3hMdR0Wu8ii85pP3lmNsLSlAhx/gWNxlJvfcKIO6lVp0Aq7iw2aIHpkbrAgMB
AAGjcjBwMAkGA1UdEwQCMAAwDgYDVR0PAQH/BAQDAgWgMBMGA1UdJQQMMAoGCCsG
AQUFBwMCMB0GA1UdDgQWBBQDEh0a24Qpb6Q9EjfOCy3N8MsfWzAfBgNVHSMEGDAW
gBSL+da7Qj6+Np3yJYsChFf4ZjintTANBgkqhkiG9w0BAQsFAAOCAQEAzALnBLwZ
g2F8uXH5HlYvBO9Nw7oyEbKm1tkpsk79D/5/lNEWzgi91W1r2mswGTFTyVBUL6PG
IoDk6Rx24LzjpkOqvCdK+GSTWgLWpYFuI92I1tFFHvX9B4KN2YcK9iUzelf70KHn
1hcgHmE0rKNuEq9SVd8ElRYiwIaT4QGuGI6QBLc65H6vDInUiOpkmRR6ZLBdL6Rf
2RAMbo51+pW/YPq2n/iy0rB4Nz4d/gSYTmPhdAJmHaWkG8DKEjOiotvWU1lQfEyt
MKKEFSuips9a4MiRo5Wy8fD+ZoSJSrT0va954M/JZHr8j8dY1P5e52z4RsGLNhxs
d2fPb1m1emXMZQ==
-----END CERTIFICATE-----"#;

    const CLIENT_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQC8s9xcSS8VS6ta
eILzzmnL0Y66JvlCHdxZCQRfboQ8pwzg7knn+R1hEVElAaV1md6wHB0chMmYEU6a
o4stVXvVkHAT94jpfHUAdi6TXH+xoiF+q6ICE/PSL7J4x/BAxxi09/XbVwYdnWJY
We4+FZrKg8hHVIImTuOZoMEn7hCG3JToZF2nqjhY++9FcakLibMFaNq/43At+n+m
jy+uqZS5cPszWAdO/W+3z5qayZwXgUlr+zOgnvOHfLgZIdiZr3dcOwR2l4IN0ZVh
xNmYQj/fE/RdhNM3hMdR0Wu8ii85pP3lmNsLSlAhx/gWNxlJvfcKIO6lVp0Aq7iw
2aIHpkbrAgMBAAECggEARs3cBLqnEoACix9Fz5JnSwVV3w5Jn5/Rsoy6Gc6/inyJ
zgpLK+Hivq2/Ozn7af1yu7TIzY8bj1YLHuX3jmqRXQhlrXBHbIh45FPz1PIzraSu
mbdvwgTXi0m/Vyd6Q+wQnrKdixADqPAJWypfROdZXdyFtRIGBba7GsVhRIjEpb0Q
pVuban3x1eCG436lwxVLgO0xeHD0x01q8ia5l2gWsg2S4vM5OhCvFBbh2MkcGRBv
yldR1lmTHJ4BGszKkHrkquKN8oMAFpJ+LYQ5yb0A1DkLk7sBzPXHbNwXMIkGJpzu
CygT2nnLFxwBEd/KSCKXdrROD2b0fWgjCdXTZBzxhQKBgQDeMDPeN/EjfBxq9A6s
n5cMMuC/j14zQeaAThNjtKy9tPF9ProbceSbFgFhLfOjVuq7PJWXA9uOCa6NlZoI
KzBZm+ZSNND1U9tI+jP3WAdjJ8HF0Aw1FgRGoFvl4ppXIjrlAuqZRGcVuWKlib67
vBRVytNliMt1Fbhwk7vxdg8FXQKBgQDZayd1Z6QB1uN0WRMPzxIx05zAWIy4igzy
akCoQwf+ZNbjlTQKDRnRbuSviTciYys9ctnL69UFmRxmGw00pU2eKtwDmVAiWIDN
fAthmmx2YNeURV9IioRtvUkHG2likiIDWwkWspXnPoy5c+sdI5dSamuj2/OJxd81
9UQSKwMw5wKBgE5FbtA2ptUYUK6AwXagVcavWatB5y5pZbkHSB9Us5G032l+onMu
oRjdHKlOVcjRwqkpA42Kh1q3IG2yKOv9wu+eUvncr0vtOY+wzIOy2A9fHwz/aH1+
/wyeSyFlvXc6kMLCT0Ck7yehAhZMuwtJi2RZqjTXhsz9VNcbxBagv1PlAoGBAMDN
gkdd6hXrfvcdSocZZRQkiPwVSm0XlxWd3cqY7szMlbdqB6TmK0ALK+byMp9e++hZ
IgTxFI4LUiDF2ncWI/egIE1ctrBOdaJDX0BllcuAY4xL3IxSsc8zLUCNMW5FEr6R
C0VChyZy1I5c2mGTv0xJrTy4/4Xsn92Uq5HE7OZ/AoGBALPhcm8n4ueOsSoOqDxo
w1DTnJ6L3l8JRBL9AYiMiKozr0U2YOz9wqR2UqCOsY7VEUYbUUNIyLLQt4bU5Gws
X7lSS07bTumHl/boXhEmDxCEgMouZfO2HLHS6EtJkhGe2i5bATsEhnM+kR9W6++c
kfXXAnwXjyrrLvFKUakhMpIQ
-----END PRIVATE KEY-----"#;

    fn temp_file(root: &std::path::Path, name: &str, contents: &str) -> PathBuf {
        let path = root.join(name);
        fs::write(&path, contents).unwrap();
        path
    }

    fn free_port() -> u16 {
        std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap()
            .port()
    }

    fn poll_once() -> String {
        let mut buffer = vec![0u8; 8192];
        let written = harmonia_frontend_poll(buffer.as_mut_ptr() as *mut c_char, buffer.len());
        if written <= 0 {
            return String::new();
        }
        String::from_utf8_lossy(&buffer[..written as usize]).to_string()
    }

    fn temp_root(prefix: &str) -> PathBuf {
        let path =
            std::env::temp_dir().join(format!("{}-{}-{}", prefix, std::process::id(), now_ms()));
        fs::create_dir_all(&path).unwrap();
        path
    }

    fn test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    async fn connect_client(
        port: u16,
        ca_path: &std::path::Path,
        client_cert_path: &std::path::Path,
        client_key_path: &std::path::Path,
    ) -> hyper::client::conn::http2::SendRequest<HttpBody> {
        let tcp = TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        let root_store = harmonia_transport_auth::load_root_store(ca_path).unwrap();
        let client_certs = harmonia_transport_auth::load_cert_chain(client_cert_path).unwrap();
        let client_key = harmonia_transport_auth::load_private_key(client_key_path).unwrap();

        let mut client_config = rustls::ClientConfig::builder()
            .with_root_certificates(root_store)
            .with_client_auth_cert(client_certs, client_key)
            .unwrap();
        client_config.alpn_protocols = vec![b"h2".to_vec()];
        let connector = TlsConnector::from(Arc::new(client_config));
        let tls = connector
            .connect(ServerName::try_from("harmonia-http2-server").unwrap(), tcp)
            .await
            .unwrap();
        let io = TokioIo::new(tls);
        let (sender, connection) = hyper::client::conn::http2::Builder::new(TokioExecutor::new())
            .handshake(io)
            .await
            .unwrap();
        tokio::spawn(async move {
            let _ = connection.await;
        });
        sender
    }

    #[test]
    fn healthcheck_and_version() {
        let _guard = test_lock();
        assert_eq!(harmonia_frontend_healthcheck(), 1);
        let version = unsafe { CStr::from_ptr(harmonia_frontend_version()) }
            .to_str()
            .unwrap();
        assert_eq!(version, "harmonia-http2-mtls/0.1.0");
    }

    #[test]
    fn http2_stream_roundtrip() {
        let _guard = test_lock();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let temp = temp_root("harmonia-http2-mtls-test");
            std::env::set_var("HARMONIA_STATE_ROOT", &temp);
            harmonia_config_store::init_v2().unwrap();

            let port = free_port();
            let bind = format!("127.0.0.1:{port}");
            let ca_path = temp_file(&temp, "ca.crt", CA_CERT);
            let server_cert_path = temp_file(&temp, "server.crt", SERVER_CERT);
            let server_key_path = temp_file(&temp, "server.key", SERVER_KEY);
            let client_cert_path = temp_file(&temp, "client.crt", CLIENT_CERT);
            let client_key_path = temp_file(&temp, "client.key", CLIENT_KEY);

            harmonia_config_store::set_config("harmonia-cli", COMPONENT, "bind", &bind).unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "ca-cert",
                &ca_path.to_string_lossy(),
            )
            .unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "server-cert",
                &server_cert_path.to_string_lossy(),
            )
            .unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "server-key",
                &server_key_path.to_string_lossy(),
            )
            .unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "trusted-client-fingerprints-json",
                "[\"ABCDEF1234567890\"]",
            )
            .unwrap();

            let config = CString::new("()").unwrap();
            assert_eq!(harmonia_frontend_init(config.as_ptr()), 0);
            tokio::time::sleep(Duration::from_millis(150)).await;

            let sender = connect_client(port, &ca_path, &client_cert_path, &client_key_path).await;
            let request = Request::post("/v1/stream/session-1/default")
                .header(CONTENT_TYPE, "application/x-ndjson")
                .body(
                    Full::new(Bytes::from_static(b"{\"payload\":\"hello\"}\n"))
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .unwrap();
            let mut response = sender.clone().send_request(request).await.unwrap();

            let mut polled = String::new();
            for _ in 0..20 {
                polled = poll_once();
                if !polled.is_empty() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }
            assert!(polled.contains("ABCDEF1234567890/session-1/default\thello\t"));
            assert!(polled.contains(":session-id \"session-1\""));
            assert!(polled.contains(":transport-security \"mtls\""));

            let route = CString::new("ABCDEF1234567890/session-1/default").unwrap();
            let payload = CString::new("world").unwrap();
            assert_eq!(harmonia_frontend_send(route.as_ptr(), payload.as_ptr()), 0);

            let body = response.body_mut();
            let mut received = String::new();
            for _ in 0..20 {
                if let Some(frame) = body.frame().await {
                    let frame = frame.unwrap();
                    if let Some(chunk) = frame.data_ref() {
                        received.push_str(&String::from_utf8_lossy(chunk));
                        break;
                    }
                }
            }
            assert!(received.contains("\"payload\":\"world\""));

            assert_eq!(harmonia_frontend_shutdown(), 0);
        });
    }

    #[test]
    fn http2_parallel_sessions_remain_isolated() {
        let _guard = test_lock();
        let runtime = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()
            .unwrap();

        runtime.block_on(async {
            let temp = temp_root("harmonia-http2-mtls-parallel");
            std::env::set_var("HARMONIA_STATE_ROOT", &temp);
            harmonia_config_store::init_v2().unwrap();

            let port = free_port();
            let bind = format!("127.0.0.1:{port}");
            let ca_path = temp_file(&temp, "ca.crt", CA_CERT);
            let server_cert_path = temp_file(&temp, "server.crt", SERVER_CERT);
            let server_key_path = temp_file(&temp, "server.key", SERVER_KEY);
            let client_cert_path = temp_file(&temp, "client.crt", CLIENT_CERT);
            let client_key_path = temp_file(&temp, "client.key", CLIENT_KEY);

            harmonia_config_store::set_config("harmonia-cli", COMPONENT, "bind", &bind).unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "ca-cert",
                &ca_path.to_string_lossy(),
            )
            .unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "server-cert",
                &server_cert_path.to_string_lossy(),
            )
            .unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "server-key",
                &server_key_path.to_string_lossy(),
            )
            .unwrap();
            harmonia_config_store::set_config(
                "harmonia-cli",
                COMPONENT,
                "trusted-client-fingerprints-json",
                "[\"ABCDEF1234567890\"]",
            )
            .unwrap();

            let config = CString::new("()").unwrap();
            assert_eq!(harmonia_frontend_init(config.as_ptr()), 0);
            tokio::time::sleep(Duration::from_millis(150)).await;

            let sender = connect_client(port, &ca_path, &client_cert_path, &client_key_path).await;

            let request_a = Request::post("/v1/stream/session-a/default")
                .header(CONTENT_TYPE, "application/x-ndjson")
                .body(
                    Full::new(Bytes::from_static(b"{\"payload\":\"alpha\"}\n"))
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .unwrap();
            let request_b = Request::post("/v1/stream/session-b/alerts")
                .header(CONTENT_TYPE, "application/x-ndjson")
                .body(
                    Full::new(Bytes::from_static(b"{\"payload\":\"beta\"}\n"))
                        .map_err(|never| match never {})
                        .boxed(),
                )
                .unwrap();

            let (response_a, response_b) = tokio::join!(
                sender.clone().send_request(request_a),
                sender.clone().send_request(request_b)
            );
            let mut response_a = response_a.unwrap();
            let mut response_b = response_b.unwrap();

            let mut collected = String::new();
            for _ in 0..30 {
                let next = poll_once();
                if !next.is_empty() {
                    collected.push_str(&next);
                }
                if collected.contains("ABCDEF1234567890/session-a/default\talpha\t")
                    && collected.contains("ABCDEF1234567890/session-b/alerts\tbeta\t")
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(25)).await;
            }

            assert!(collected.contains("ABCDEF1234567890/session-a/default\talpha\t"));
            assert!(collected.contains("ABCDEF1234567890/session-b/alerts\tbeta\t"));

            let route_a = CString::new("ABCDEF1234567890/session-a/default").unwrap();
            let route_b = CString::new("ABCDEF1234567890/session-b/alerts").unwrap();
            let payload_a = CString::new("reply-a").unwrap();
            let payload_b = CString::new("reply-b").unwrap();
            assert_eq!(
                harmonia_frontend_send(route_a.as_ptr(), payload_a.as_ptr()),
                0
            );
            assert_eq!(
                harmonia_frontend_send(route_b.as_ptr(), payload_b.as_ptr()),
                0
            );

            let body_a = response_a.body_mut();
            let body_b = response_b.body_mut();
            let read_a = async {
                let mut received = String::new();
                for _ in 0..20 {
                    if let Some(frame) = body_a.frame().await {
                        let frame = frame.unwrap();
                        if let Some(chunk) = frame.data_ref() {
                            received.push_str(&String::from_utf8_lossy(chunk));
                            break;
                        }
                    }
                }
                received
            };
            let read_b = async {
                let mut received = String::new();
                for _ in 0..20 {
                    if let Some(frame) = body_b.frame().await {
                        let frame = frame.unwrap();
                        if let Some(chunk) = frame.data_ref() {
                            received.push_str(&String::from_utf8_lossy(chunk));
                            break;
                        }
                    }
                }
                received
            };
            let (received_a, received_b) = tokio::join!(read_a, read_b);

            assert!(received_a.contains("\"payload\":\"reply-a\""));
            assert!(received_b.contains("\"payload\":\"reply-b\""));

            assert_eq!(harmonia_frontend_shutdown(), 0);
        });
    }
}
