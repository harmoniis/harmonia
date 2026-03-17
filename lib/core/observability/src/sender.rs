//! Background batch sender thread.
//!
//! Receives TraceMessages via bounded mpsc channel, buffers them, and flushes
//! to LangSmith in batches (every 2s or when 20 items accumulate).
//! Uses std::sync::mpsc — no tokio needed.

use crate::client::LangSmithClient;
use crate::model::{TraceMessage, new_uuid, now_iso, dotted_order_root};
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 20;
const FLUSH_INTERVAL: Duration = Duration::from_secs(2);
const CHANNEL_CAPACITY: usize = 1000;

static SENDER: OnceLock<SyncSender<TraceMessage>> = OnceLock::new();

/// Initialize the background sender thread. Call once during init.
pub(crate) fn start(api_url: &str, api_key: &str, project_name: &str) {
    let (tx, rx) = mpsc::sync_channel::<TraceMessage>(CHANNEL_CAPACITY);

    let client = LangSmithClient::new(api_url, api_key);
    let project = project_name.to_string();

    thread::Builder::new()
        .name("harmonia-observability-sender".into())
        .spawn(move || sender_loop(rx, client, &project))
        .expect("failed to spawn observability sender thread");

    let _ = SENDER.set(tx);
}

/// Send a trace message to the background thread. Non-blocking.
/// Returns false if the channel is full (message dropped).
pub(crate) fn send(msg: TraceMessage) -> bool {
    if let Some(tx) = SENDER.get() {
        tx.try_send(msg).is_ok()
    } else {
        false
    }
}

/// Flush pending traces synchronously (blocks until flush completes).
pub(crate) fn flush() {
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(TraceMessage::Flush);
        // Brief wait for flush to complete
        thread::sleep(Duration::from_millis(100));
    }
}

/// Shut down the sender thread.
pub(crate) fn shutdown() {
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(TraceMessage::Shutdown);
    }
}

fn sender_loop(rx: Receiver<TraceMessage>, client: LangSmithClient, _project: &str) {
    let mut creates: Vec<Value> = Vec::new();
    let mut updates: Vec<Value> = Vec::new();
    let mut last_flush = Instant::now();

    loop {
        // Block with timeout so we flush periodically even without new messages
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(TraceMessage::StartRun(span)) => {
                creates.push(LangSmithClient::span_to_create(&span));
            }
            Ok(TraceMessage::EndRun {
                run_id,
                status,
                outputs,
                end_time,
            }) => {
                updates.push(LangSmithClient::build_update(
                    &run_id, &status, &outputs, &end_time,
                ));
            }
            Ok(TraceMessage::Event(event)) => {
                // Fire-and-forget: create a completed run
                let run_id = new_uuid();
                let now = now_iso();
                let trace_id = event.trace_id.unwrap_or_else(|| run_id.clone());
                let dotted = event.dotted_order.unwrap_or_else(dotted_order_root);
                let mut run = json!({
                    "id": run_id,
                    "name": event.name,
                    "run_type": event.run_type,
                    "start_time": &now,
                    "end_time": &now,
                    "inputs": event.metadata,
                    "outputs": {},
                    "status": "success",
                    "trace_id": trace_id,
                    "dotted_order": dotted,
                    "session_name": event.project_name,
                });
                if let Some(ref parent) = event.parent_run_id {
                    run["parent_run_id"] = json!(parent);
                }
                creates.push(run);
            }
            Ok(TraceMessage::Flush) => {
                do_flush(&client, &mut creates, &mut updates);
                last_flush = Instant::now();
            }
            Ok(TraceMessage::Shutdown) => {
                do_flush(&client, &mut creates, &mut updates);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                do_flush(&client, &mut creates, &mut updates);
                return;
            }
        }

        // Batch flush on size or time
        let should_flush =
            creates.len() + updates.len() >= BATCH_SIZE || last_flush.elapsed() >= FLUSH_INTERVAL;
        if should_flush && (!creates.is_empty() || !updates.is_empty()) {
            do_flush(&client, &mut creates, &mut updates);
            last_flush = Instant::now();
        }
    }
}

fn do_flush(client: &LangSmithClient, creates: &mut Vec<Value>, updates: &mut Vec<Value>) {
    if creates.is_empty() && updates.is_empty() {
        return;
    }

    // Silently drop on error — observability must never block the agent
    let _ = client.post_runs_batch(creates, updates);

    creates.clear();
    updates.clear();
}
