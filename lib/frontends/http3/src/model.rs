use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use bytes::Bytes;
use tokio::sync::mpsc;

pub(crate) const COMPONENT: &str = "http3-frontend";
pub(crate) const DEFAULT_BIND: &str = "127.0.0.1:9443";
pub(crate) const DEFAULT_MAX_STREAMS: u32 = 64;
pub(crate) const DEFAULT_IDLE_TIMEOUT_MS: u64 = 300_000;
pub(crate) const DEFAULT_MAX_FRAME_BYTES: usize = 64 * 1024;
pub(crate) const LINEAGE_SYMBOL: &str = "http3_tls_master_seed";

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

#[derive(Clone)]
pub(crate) struct FrontendConfig {
    pub(crate) bind: std::net::SocketAddr,
    pub(crate) ca_cert: PathBuf,
    pub(crate) server_cert: PathBuf,
    pub(crate) server_key: PathBuf,
    pub(crate) trusted_fingerprints: Arc<HashSet<String>>,
    pub(crate) max_concurrent_streams: u32,
    pub(crate) session_idle_timeout_ms: u64,
    pub(crate) max_frame_bytes: usize,
}

#[derive(Clone)]
pub(crate) struct VerifiedPeer {
    pub(crate) identity_fingerprint: String,
    pub(crate) cert_fingerprint: String,
}

pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub(crate) fn escape_metadata(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
