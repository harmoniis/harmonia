use bytes::Bytes;
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, RwLock};
use std::time::Duration;
use tokio::sync::{mpsc, watch};

use crate::model::{now_ms, set_error, state_slot, FrontendState, SessionHandle};
use crate::server::run_server;
use crate::tls::load_config;

pub(crate) async fn session_cleanup_loop(
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    route: String,
    sender: mpsc::Sender<Bytes>,
    last_activity_ms: Arc<std::sync::atomic::AtomicU64>,
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

pub(crate) fn take_state() -> Option<FrontendState> {
    state_slot().lock().ok().and_then(|mut guard| guard.take())
}

pub(crate) fn enqueue_outbound(
    outbound: &mpsc::Sender<Bytes>,
    payload: Bytes,
) -> Result<(), String> {
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

pub(crate) fn start_server() -> Result<FrontendState, String> {
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

pub(crate) fn shutdown_state(mut state: FrontendState) {
    let _ = state.shutdown.send(true);
    if let Some(handle) = state.server_thread.take() {
        let _ = handle.join();
    }
}
