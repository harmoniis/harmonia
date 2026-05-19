//! [`Http3Frontend`] — actor-owned HTTP/3 mTLS frontend.
//!
//! All transport state (the running quinn endpoint task, the session map,
//! the inbound queue) lives on `Self`. There are no module-level singletons,
//! and the runtime treats this frontend identically to Slack/Telegram/MQTT/
//! TUI through the [`Frontend`] trait.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock};

use async_trait::async_trait;
use bytes::Bytes;
use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};
use tokio::sync::watch;

use crate::model::{now_ms, FrontendConfig, InboundSignal, SessionHandle};
use crate::server;
use crate::tls;

pub struct Http3Frontend {
    inbound: Arc<Mutex<VecDeque<InboundSignal>>>,
    sessions: Arc<RwLock<HashMap<String, SessionHandle>>>,
    shutdown_tx: watch::Sender<bool>,
    server_handle: tokio::task::JoinHandle<Result<(), String>>,
}

#[async_trait]
impl Frontend for Http3Frontend {
    type Config = ();

    fn name() -> &'static str {
        "http3"
    }

    fn security_label() -> &'static str {
        // mTLS-authenticated remote operator. Phase 5b will lift this to
        // "owner" once the PGP-Hello handshake is in place — until then,
        // treat it as authenticated, same as messaging frontends.
        "authenticated"
    }

    async fn init(_: ()) -> Result<Self, String> {
        let config: FrontendConfig = tls::load_config()?;
        let inbound = Arc::new(Mutex::new(VecDeque::new()));
        let sessions = Arc::new(RwLock::new(HashMap::new()));
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel::<Result<(), String>>(1);

        let inbound_for_task = inbound.clone();
        let sessions_for_task = sessions.clone();
        let server_handle = tokio::spawn(async move {
            server::run_server(
                config,
                inbound_for_task,
                sessions_for_task,
                shutdown_rx,
                ready_tx,
            )
            .await
        });

        // Block on readiness so init() either succeeds with a bound endpoint
        // or fails with the bind error — never silently leaves a half-up
        // server in the background.
        let ready_result = tokio::task::spawn_blocking(move || {
            ready_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .map_err(|e| format!("http3 server ready timeout: {e}"))
        })
        .await
        .map_err(|e| format!("ready join failed: {e}"))??;
        ready_result?;

        eprintln!("[INFO] [http3] endpoint ready");
        Ok(Self {
            inbound,
            sessions,
            shutdown_tx,
            server_handle,
        })
    }

    async fn poll(&mut self) -> PollResult {
        let mut out = Vec::new();
        if let Ok(mut q) = self.inbound.lock() {
            while let Some(signal) = q.pop_front() {
                out.push(InboundMessage {
                    address: signal.sub_channel,
                    text: signal.payload,
                    metadata: Some(signal.metadata),
                });
            }
        }
        Ok(out)
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        let handle = self
            .sessions
            .read()
            .ok()
            .and_then(|g| g.get(channel).cloned());
        let Some(handle) = handle else {
            return Err(format!("no active HTTP/3 stream for route {channel}"));
        };
        let envelope = serde_json::to_string(&serde_json::json!({ "payload": text }))
            .map_err(|e| format!("serialize outbound payload failed: {e}"))?
            + "\n";
        server::enqueue_outbound(&handle.outbound, Bytes::from(envelope))?;
        handle
            .last_activity_ms
            .store(now_ms(), std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }

    async fn shutdown(&mut self) {
        let _ = self.shutdown_tx.send(true);
        if !self.server_handle.is_finished() {
            // Give the server task a moment to drain.
            let _ = tokio::time::timeout(
                std::time::Duration::from_secs(2),
                &mut self.server_handle,
            )
            .await;
        }
        if let Ok(mut g) = self.sessions.write() {
            g.clear();
        }
    }
}
