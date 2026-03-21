use std::sync::Arc;

use crate::model::{
    FrontendConfig, COMPONENT, DEFAULT_BIND, DEFAULT_IDLE_TIMEOUT_MS, DEFAULT_MAX_FRAME_BYTES,
    DEFAULT_MAX_STREAMS, LINEAGE_SYMBOL,
};

pub(crate) fn load_config() -> Result<FrontendConfig, String> {
    harmonia_config_store::init().map_err(|e| format!("config-store init failed: {e}"))?;
    let _ = harmonia_transport_auth::record_tls_lineage_seed(COMPONENT, "tls", LINEAGE_SYMBOL);

    let bind = harmonia_config_store::get_own_or(COMPONENT, "bind", DEFAULT_BIND)
        .unwrap_or_else(|_| DEFAULT_BIND.to_string())
        .parse::<std::net::SocketAddr>()
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
