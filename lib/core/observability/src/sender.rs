//! Background batch sender thread.
//!
//! Buffers trace events and flushes to LangSmith in batches.
//! Exponential backoff on 429 — never hammers a rate-limited endpoint.
//! Silent on success, vocal on error, quiet during backoff.

use crate::client::{FlushResult, LangSmithClient};
use crate::model::{dotted_order_child, dotted_order_for, new_uuid, now_iso, TraceMessage};
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 50;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const CHANNEL_CAPACITY: usize = 1000;
const BACKOFF_INITIAL: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_secs(60);

static SENDER: OnceLock<SyncSender<TraceMessage>> = OnceLock::new();

/// Start the sender thread. Idempotent — second call is a no-op.
pub(crate) fn start(api_url: &str, api_key: &str, project_name: &str) {
    if SENDER.get().is_some() {
        return;
    }
    let (tx, rx) = mpsc::sync_channel::<TraceMessage>(CHANNEL_CAPACITY);
    let client = LangSmithClient::new(api_url, api_key);
    let project = project_name.to_string();
    thread::Builder::new()
        .name("harmonia-observability-sender".into())
        .spawn(move || sender_loop(rx, client, &project))
        .expect("failed to spawn observability sender thread");
    let _ = SENDER.set(tx);
}

/// Non-blocking send. Returns false if channel full (dropped).
pub(crate) fn send(msg: TraceMessage) -> bool {
    SENDER
        .get()
        .map(|tx| tx.try_send(msg).is_ok())
        .unwrap_or(false)
}

/// Flush pending traces (brief blocking wait).
pub(crate) fn flush() {
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(TraceMessage::Flush);
        thread::sleep(Duration::from_millis(50));
    }
}

/// Signal the sender thread to shut down.
pub(crate) fn shutdown() {
    if let Some(tx) = SENDER.get() {
        let _ = tx.try_send(TraceMessage::Shutdown);
    }
}

fn sender_loop(rx: Receiver<TraceMessage>, client: LangSmithClient, _project: &str) {
    let mut creates: Vec<Value> = Vec::new();
    let mut updates: Vec<Value> = Vec::new();
    let mut last_flush = Instant::now();
    let mut backoff = Duration::ZERO;
    let mut backoff_until = Instant::now();
    let mut backoff_logged = false;

    loop {
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
                let run_id = new_uuid();
                let now = now_iso();
                let trace_id = event.trace_id.unwrap_or_else(|| run_id.clone());
                let dotted = event
                    .dotted_order
                    .map(|d| dotted_order_child(&d, &run_id))
                    .unwrap_or_else(|| dotted_order_for(&run_id));
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
                if Instant::now() >= backoff_until {
                    do_flush(
                        &client,
                        &mut creates,
                        &mut updates,
                        &mut backoff,
                        &mut backoff_until,
                        &mut backoff_logged,
                    );
                    last_flush = Instant::now();
                }
            }
            Ok(TraceMessage::Shutdown) => {
                // Best-effort final drain, ignore backoff
                let _ = client.post_runs_batch(&creates, &updates);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = client.post_runs_batch(&creates, &updates);
                return;
            }
        }

        // During backoff: keep buffering, don't flush
        if Instant::now() < backoff_until {
            // Shed excess if buffer grows too large during backoff
            if creates.len() + updates.len() > CHANNEL_CAPACITY {
                let excess = (creates.len() + updates.len()) - CHANNEL_CAPACITY;
                creates.drain(..excess.min(creates.len()));
            }
            continue;
        }

        let pending = creates.len() + updates.len();
        let should_flush = pending >= BATCH_SIZE || last_flush.elapsed() >= FLUSH_INTERVAL;
        if should_flush && pending > 0 {
            do_flush(
                &client,
                &mut creates,
                &mut updates,
                &mut backoff,
                &mut backoff_until,
                &mut backoff_logged,
            );
            last_flush = Instant::now();
        }
    }
}

fn do_flush(
    client: &LangSmithClient,
    creates: &mut Vec<Value>,
    updates: &mut Vec<Value>,
    backoff: &mut Duration,
    backoff_until: &mut Instant,
    backoff_logged: &mut bool,
) {
    if creates.is_empty() && updates.is_empty() {
        return;
    }

    match client.post_runs_batch(creates, updates) {
        FlushResult::Ok => {
            if *backoff > Duration::ZERO {
                eprintln!("[INFO] [observability] LangSmith recovered from rate limit, resuming");
            }
            *backoff = Duration::ZERO;
            *backoff_logged = false;
            creates.clear();
            updates.clear();
        }
        FlushResult::RateLimited(body) => {
            *backoff = if backoff.is_zero() {
                BACKOFF_INITIAL
            } else {
                (*backoff * 2).min(BACKOFF_MAX)
            };
            *backoff_until = Instant::now() + *backoff;
            if !*backoff_logged {
                // Log response body once — shows plan limits, quota details
                let detail = if body.is_empty() {
                    String::new()
                } else {
                    let clipped = if body.len() > 200 {
                        &body[..200]
                    } else {
                        &body
                    };
                    format!(" ({})", clipped)
                };
                eprintln!(
                    "[WARN] [observability] LangSmith 429 — backing off {}s ({} items buffered){}",
                    backoff.as_secs(),
                    creates.len() + updates.len(),
                    detail
                );
                *backoff_logged = true;
            }
        }
        FlushResult::Error(e) => {
            eprintln!("[WARN] [observability] LangSmith flush error: {e}");
            // Drop on non-retryable errors
            creates.clear();
            updates.clear();
        }
    }
}
