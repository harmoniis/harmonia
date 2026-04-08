//! ObservabilityActor — single trace sink.
//!
//! All trace data flows as ractor cast (fire-and-forget).
//! Owns the sender thread, sampling decisions, and parent->child correlation.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::SyncSender;
use std::time::{SystemTime, UNIX_EPOCH};

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_observability::model::{
    dotted_order_child, dotted_order_for, now_iso, DottedOrderEntry, TraceEvent, TraceLevel,
    TraceMessage, TraceSpan,
};
use harmonia_observability::{ObsMsg, ObservabilityConfig};

pub struct ObservabilityActor;

const MAX_REJECTED_TRACES: usize = 2048;
/// TTL for dotted_order entries (5 minutes in seconds).
const DOTTED_ORDER_TTL_SECS: u64 = 300;

pub struct ObsActorState {
    pub sender: Option<SyncSender<TraceMessage>>,
    pub config: ObservabilityConfig,
    /// Parent->child dotted_order correlation with timestamps for TTL eviction.
    dotted_orders: HashMap<String, (DottedOrderEntry, u64)>,
    /// Active sampled-in trace_ids (root spans).
    active_traces: HashSet<String>,
    /// Rejected (sampled-out) trace_ids — ring buffer eviction (no clear-all).
    rejected_traces: HashSet<String>,
    /// FIFO order for rejected_traces eviction.
    rejected_order: std::collections::VecDeque<String>,
    /// Counter for periodic TTL sweeps (every 256 span starts).
    span_counter: u64,
}

impl ObsActorState {
    fn handle_span_start(
        &mut self,
        run_id: String,
        parent_run_id: Option<String>,
        trace_id: Option<String>,
        name: String,
        run_type: String,
        metadata: serde_json::Value,
    ) {
        if !self.config.enabled {
            return;
        }
        let sender = match &self.sender {
            Some(s) => s,
            None => return,
        };

        let is_root = parent_run_id.is_none() && trace_id.is_none();

        if is_root {
            // Deterministic hash-based sampling: consistent per trace, no timing bias.
            if self.config.sample_rate < 1.0 {
                let hash = crate::ipc::fnv1a_64(run_id.as_bytes());
                let roll: f64 = (hash % 10000) as f64 / 10000.0;
                if roll >= self.config.sample_rate {
                    // Ring buffer eviction: remove oldest before inserting new.
                    if self.rejected_traces.len() >= MAX_REJECTED_TRACES {
                        if let Some(oldest) = self.rejected_order.pop_front() {
                            self.rejected_traces.remove(&oldest);
                        }
                    }
                    self.rejected_traces.insert(run_id.clone());
                    self.rejected_order.push_back(run_id);
                    return;
                }
            }
            self.active_traces.insert(run_id.clone());

            // Periodic TTL sweep of dotted_orders (every 256 root spans)
            self.span_counter += 1;
            if self.span_counter % 256 == 0 {
                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                self.dotted_orders
                    .retain(|_, (_, ts)| now.saturating_sub(*ts) < DOTTED_ORDER_TTL_SECS);
            }
        } else {
            // Child span: check if parent trace was sampled out
            if let Some(ref tid) = trace_id {
                if self.rejected_traces.contains(tid) {
                    return;
                }
            }
            if let Some(ref pid) = parent_run_id {
                if self.rejected_traces.contains(pid) {
                    return;
                }
            }
        }

        let actual_trace_id = trace_id.unwrap_or_else(|| run_id.clone());

        let dotted_order = if let Some(ref parent_rid) = parent_run_id {
            if let Some((parent_entry, _)) = self.dotted_orders.get(parent_rid) {
                dotted_order_child(&parent_entry.dotted_order, &run_id)
            } else {
                dotted_order_for(&run_id)
            }
        } else {
            dotted_order_for(&run_id)
        };

        let now_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.dotted_orders.insert(
            run_id.clone(),
            (
                DottedOrderEntry {
                    dotted_order: dotted_order.clone(),
                    trace_id: actual_trace_id.clone(),
                },
                now_secs,
            ),
        );

        let _ = sender.try_send(TraceMessage::StartRun(TraceSpan {
            run_id,
            parent_run_id,
            trace_id: actual_trace_id,
            dotted_order,
            name,
            run_type,
            start_time: now_iso(),
            end_time: None,
            status: None,
            inputs: metadata,
            outputs: None,
            extra: json!({}),
            project_name: self.config.project_name.clone(),
        }));
    }

    fn handle_span_end(&mut self, run_id: String, status: String, outputs: serde_json::Value) {
        if !self.config.enabled {
            return;
        }
        let sender = match &self.sender {
            Some(s) => s,
            None => return,
        };

        if self.rejected_traces.remove(&run_id) {
            return;
        }

        self.active_traces.remove(&run_id);
        self.dotted_orders.remove(&run_id);

        let _ = sender.try_send(TraceMessage::EndRun {
            run_id,
            status,
            outputs,
            end_time: now_iso(),
        });
    }

    fn handle_event(
        &mut self,
        name: String,
        run_type: String,
        metadata: serde_json::Value,
        parent_run_id: Option<String>,
        trace_id: Option<String>,
    ) {
        if !self.config.enabled {
            return;
        }
        let sender = match &self.sender {
            Some(s) => s,
            None => return,
        };

        // Check if parent was sampled out
        if let Some(ref tid) = trace_id {
            if self.rejected_traces.contains(tid) {
                return;
            }
        }
        if let Some(ref pid) = parent_run_id {
            if self.rejected_traces.contains(pid) {
                return;
            }
        }

        let (actual_trace_id, actual_dotted_order) = if let Some(ref pid) = parent_run_id {
            if let Some((entry, _)) = self.dotted_orders.get(pid) {
                (
                    Some(entry.trace_id.clone()),
                    Some(entry.dotted_order.clone()),
                )
            } else {
                (trace_id, None)
            }
        } else {
            (trace_id, None)
        };

        let _ = sender.try_send(TraceMessage::Event(TraceEvent {
            name,
            run_type,
            metadata,
            project_name: self.config.project_name.clone(),
            trace_id: actual_trace_id,
            parent_run_id,
            dotted_order: actual_dotted_order,
        }));
    }
}

impl Actor for ObservabilityActor {
    type Msg = ObsMsg;
    type State = ObsActorState;
    type Arguments = (
        Option<SyncSender<TraceMessage>>,
        Option<ObservabilityConfig>,
    );

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (sender, config): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let config = config.unwrap_or_default();
        eprintln!(
            "[INFO] [runtime] ObservabilityActor started (level={}, sample_rate={})",
            config.trace_level.as_str(),
            config.sample_rate
        );
        Ok(ObsActorState {
            sender,
            config,
            dotted_orders: HashMap::with_capacity(256),
            active_traces: HashSet::with_capacity(64),
            rejected_traces: HashSet::with_capacity(256),
            rejected_order: std::collections::VecDeque::with_capacity(256),
            span_counter: 0,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ObsMsg::SpanStart {
                run_id,
                parent_run_id,
                trace_id,
                name,
                run_type,
                metadata,
            } => {
                state.handle_span_start(run_id, parent_run_id, trace_id, name, run_type, metadata);
            }
            ObsMsg::SpanEnd {
                run_id,
                status,
                outputs,
            } => {
                state.handle_span_end(run_id, status, outputs);
            }
            ObsMsg::Event {
                name,
                run_type,
                metadata,
                parent_run_id,
                trace_id,
            } => {
                state.handle_event(name, run_type, metadata, parent_run_id, trace_id);
            }
            ObsMsg::Flush => {
                if let Some(ref sender) = state.sender {
                    let _ = sender.try_send(TraceMessage::Flush);
                }
            }
            ObsMsg::Shutdown => {
                if let Some(ref sender) = state.sender {
                    let _ = sender.try_send(TraceMessage::Flush);
                    // Yield to tokio instead of blocking the runtime thread.
                    // The flush message is already queued; the sender thread
                    // will process it before the shutdown message.
                    tokio::task::yield_now().await;
                    let _ = sender.try_send(TraceMessage::Shutdown);
                }
                eprintln!("[INFO] [runtime] ObservabilityActor shutting down");
            }
            ObsMsg::Reconfigure {
                trace_level,
                sample_rate,
                enabled,
            } => {
                if let Some(level_str) = trace_level {
                    state.config.trace_level = TraceLevel::from_str(&level_str);
                }
                if let Some(rate) = sample_rate {
                    state.config.sample_rate = rate.clamp(0.0, 1.0);
                }
                if let Some(en) = enabled {
                    state.config.enabled = en;
                }
            }
        }
        Ok(())
    }
}
