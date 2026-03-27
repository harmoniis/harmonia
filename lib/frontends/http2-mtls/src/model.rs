use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use std::collections::{HashMap, VecDeque};
use std::convert::Infallible;
use std::ffi::CStr;
use std::os::raw::c_char;
use std::sync::atomic::AtomicU64;
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::thread::JoinHandle;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, watch};

pub(crate) const VERSION: &[u8] = b"harmonia-http2-mtls/0.1.0\0";
pub(crate) const COMPONENT: &str = "http2-frontend";
pub(crate) const DEFAULT_BIND: &str = "127.0.0.1:9443";
pub(crate) const DEFAULT_MAX_STREAMS: u32 = 64;
pub(crate) const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
pub(crate) const DEFAULT_MAX_FRAME_BYTES: usize = 64 * 1024;
pub(crate) const LINEAGE_SYMBOL: &str = "http2_tls_master_seed";

pub(crate) type HttpBody = http_body_util::combinators::BoxBody<Bytes, Infallible>;

#[derive(Clone)]
pub(crate) struct InboundSignal {
    pub(crate) sub_channel: String,
    pub(crate) payload: String,
    pub(crate) metadata: String,
}

#[derive(Clone)]
pub(crate) struct SessionHandle {
    pub(crate) outbound: mpsc::Sender<Bytes>,
    pub(crate) last_activity_ms: Arc<AtomicU64>,
}

pub(crate) struct FrontendState {
    pub(crate) inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    pub(crate) sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    pub(crate) shutdown: watch::Sender<bool>,
    pub(crate) server_thread: Option<JoinHandle<()>>,
}

#[derive(Clone)]
pub(crate) struct FrontendConfig {
    pub(crate) bind: std::net::SocketAddr,
    pub(crate) ca_cert: std::path::PathBuf,
    pub(crate) server_cert: std::path::PathBuf,
    pub(crate) server_key: std::path::PathBuf,
    pub(crate) trusted_fingerprints: Arc<std::collections::HashSet<String>>,
    pub(crate) max_concurrent_streams: u32,
    pub(crate) session_idle_timeout_ms: u64,
    pub(crate) max_frame_bytes: usize,
}

#[derive(Clone)]
pub(crate) struct VerifiedPeer {
    pub(crate) identity_fingerprint: String,
    pub(crate) cert_fingerprint: String,
}

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();
static STATE: OnceLock<Mutex<Option<FrontendState>>> = OnceLock::new();

pub(crate) fn last_error() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

pub(crate) fn state_slot() -> &'static Mutex<Option<FrontendState>> {
    STATE.get_or_init(|| Mutex::new(None))
}

pub(crate) fn set_error(message: impl Into<String>) {
    if let Ok(mut guard) = last_error().write() {
        *guard = message.into();
    }
}

pub(crate) fn clear_error() {
    if let Ok(mut guard) = last_error().write() {
        guard.clear();
    }
}

pub(crate) fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    let raw = unsafe { CStr::from_ptr(ptr) };
    Ok(raw.to_string_lossy().into_owned())
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn boxed_full(payload: impl Into<Bytes>) -> HttpBody {
    Full::new(payload.into())
        .map_err(|never| match never {})
        .boxed()
}

pub(crate) fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
