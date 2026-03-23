//! Background batch sender thread.
//!
//! Buffers trace events and flushes via the configured TraceBackend provider.
//! Exponential backoff on 429 — never hammers a rate-limited endpoint.

use crate::backend::{FlushResult, TraceBackend};
use crate::model::{dotted_order_child, dotted_order_for, new_uuid, now_iso, TraceMessage};
use crate::providers::langsmith::LangSmith;
use serde_json::{json, Value};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

const BATCH_SIZE: usize = 50;
const FLUSH_INTERVAL: Duration = Duration::from_secs(5);
const CHANNEL_CAPACITY: usize = 1000;
const BACKOFF_INITIAL: Duration = Duration::from_secs(5);
const BACKOFF_MAX: Duration = Duration::from_secs(60);

/// Start the sender thread with the given provider.
pub fn start_with_backend(provider: Box<dyn TraceBackend>) -> SyncSender<TraceMessage> {
    let (tx, rx) = mpsc::sync_channel::<TraceMessage>(CHANNEL_CAPACITY);
    thread::Builder::new()
        .name("harmonia-observability-sender".into())
        .spawn(move || sender_loop(rx, provider))
        .expect("failed to spawn observability sender thread");
    tx
}

fn sender_loop(rx: Receiver<TraceMessage>, provider: Box<dyn TraceBackend>) {
    let provider_name = provider.name();
    let mut creates: Vec<Value> = Vec::new();
    let mut updates: Vec<Value> = Vec::new();
    let mut last_flush = Instant::now();
    let mut backoff = Duration::ZERO;
    let mut backoff_until = Instant::now();
    let mut backoff_logged = false;

    loop {
        match rx.recv_timeout(Duration::from_millis(500)) {
            Ok(TraceMessage::StartRun(span)) => {
                creates.push(LangSmith::span_to_create(&span));
            }
            Ok(TraceMessage::EndRun {
                run_id,
                status,
                outputs,
                end_time,
            }) => {
                updates.push(LangSmith::build_update(&run_id, &status, &outputs, &end_time));
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
                        provider.as_ref(),
                        provider_name,
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
                let _ = provider.submit_batch(&creates, &updates);
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                let _ = provider.submit_batch(&creates, &updates);
                return;
            }
        }

        if Instant::now() < backoff_until {
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
                provider.as_ref(),
                provider_name,
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
    provider: &dyn TraceBackend,
    provider_name: &str,
    creates: &mut Vec<Value>,
    updates: &mut Vec<Value>,
    backoff: &mut Duration,
    backoff_until: &mut Instant,
    backoff_logged: &mut bool,
) {
    if creates.is_empty() && updates.is_empty() {
        return;
    }

    let n_creates = creates.len();
    let n_updates = updates.len();

    match provider.submit_batch(creates, updates) {
        FlushResult::Ok => {
            if *backoff > Duration::ZERO {
                eprintln!("[INFO] [observability] {} recovered from rate limit", provider_name);
            }
            eprintln!(
                "[INFO] [observability] Flushed {} creates, {} updates to {}",
                n_creates, n_updates, provider_name
            );
            *backoff = Duration::ZERO;
            *backoff_logged = false;
            creates.clear();
            updates.clear();
        }
        FlushResult::RateLimited(body) => {
            *backoff = if backoff.is_zero() { BACKOFF_INITIAL } else { (*backoff * 2).min(BACKOFF_MAX) };
            *backoff_until = Instant::now() + *backoff;
            if !*backoff_logged {
                let detail = if body.len() > 200 { &body[..200] } else { &body };
                eprintln!(
                    "[WARN] [observability] {} 429 — backing off {}s ({} buffered) {}",
                    provider_name, backoff.as_secs(), creates.len() + updates.len(), detail
                );
                *backoff_logged = true;
            }
        }
        FlushResult::Error(e) => {
            eprintln!("[WARN] [observability] {} flush error: {e}", provider_name);
            creates.clear();
            updates.clear();
        }
    }
}
