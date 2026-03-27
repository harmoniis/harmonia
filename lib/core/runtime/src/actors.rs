//! Actor wrappers for Harmonia components.
//!
//! Each wrapper is a ractor actor that owns a component's lifecycle.
//! pre_start initializes the component, handle() dispatches messages
//! to the component's public API, and supervision handles recovery.

use std::collections::{HashMap, HashSet};
use std::sync::mpsc::SyncSender;
use std::time::{SystemTime, UNIX_EPOCH};

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};
use harmonia_observability::model::{
    dotted_order_child, dotted_order_for, now_iso, DottedOrderEntry, TraceEvent, TraceLevel,
    TraceMessage, TraceSpan,
};
use harmonia_observability::{ObsMsg, ObservabilityConfig, Traceable};

use crate::msg::BridgeMsg;

// ── Shared message type for all component actors ─────────────────────

use ractor::RpcReplyPort;

pub enum ComponentMsg {
    /// Periodic tick — poll, flush, or heartbeat.
    Tick,
    /// Process an inbound signal.
    Signal { payload_sexp: String },
    /// Dispatch a component command (sexp in, sexp out).
    /// Runs in the component actor's mailbox — never blocks the supervisor.
    Dispatch(String, RpcReplyPort<String>),
    /// Graceful shutdown.
    Shutdown,
}

// ── ChronicleActor ───────────────────────────────────────────────────

pub struct ChronicleActor;

impl Actor for ChronicleActor {
    type Msg = ComponentMsg;
    type State = bool; // initialized
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_chronicle::init();
        eprintln!("[INFO] [runtime] ChronicleActor started");
        Ok(true)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let deleted = harmonia_chronicle::gc().unwrap_or(0);
                if deleted > 0 {
                    if let Some(obs) = harmonia_observability::get_obs_actor() {
                        if harmonia_observability::harmonia_observability_is_standard() {
                            let obs_opt: Option<ractor::ActorRef<ObsMsg>> = Some(obs.clone());
                            obs_opt.trace_event(
                                "chronicle-gc",
                                "tool",
                                json!({"rows_deleted": deleted}),
                            );
                        }
                    }
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("chronicle", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ChronicleActor shutting down");
            }
            ComponentMsg::Signal { .. } => { /* chronicle does not handle signals */ }
        }
        Ok(())
    }
}

// ── TailnetActor ─────────────────────────────────────────────────────

pub struct TailnetActor;

pub struct TailnetState {
    bridge: ActorRef<BridgeMsg>,
    #[allow(dead_code)]
    obs: Option<ActorRef<ObsMsg>>,
}

impl Actor for TailnetActor {
    type Msg = ComponentMsg;
    type State = TailnetState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_tailnet::transport::start_listener();
        eprintln!("[INFO] [runtime] TailnetActor started");
        Ok(TailnetState { bridge, obs })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let messages = harmonia_tailnet::transport::poll_messages();
                for mesh_msg in messages {
                    let msg = HarmoniaMessage {
                        id: 0,
                        source: 0,
                        target: 0,
                        kind: ActorKind::Tailnet,
                        timestamp: now_unix(),
                        payload: MessagePayload::MeshInbound {
                            from_node: mesh_msg.from.to_string(),
                            msg_type: format!("{:?}", mesh_msg.msg_type),
                            payload: mesh_msg.payload,
                        },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("tailnet", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Signal { payload_sexp } => {
                // Parse and send mesh message (best-effort)
                let _ = payload_sexp; // TODO: parse destination and message from sexp
            }
            ComponentMsg::Shutdown => {
                harmonia_tailnet::transport::stop_listener();
                eprintln!("[INFO] [runtime] TailnetActor shutting down");
            }
        }
        Ok(())
    }
}

// ── SignalogradActor ─────────────────────────────────────────────────
//
// Signalograd exposes C-style functions (from legacy FFI) that now
// compile as regular Rust pub fn. We wrap them with safe helpers.

pub struct SignalogradActor;

pub struct SignalogradState {
    bridge: ActorRef<BridgeMsg>,
    #[allow(dead_code)]
    obs: Option<ActorRef<ObsMsg>>,
}

impl Actor for SignalogradActor {
    type Msg = ComponentMsg;
    type State = SignalogradState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        harmonia_signalograd::harmonia_signalograd_init();
        eprintln!("[INFO] [runtime] SignalogradActor started");
        Ok(SignalogradState { bridge, obs })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                let status_ptr = harmonia_signalograd::harmonia_signalograd_status();
                if !status_ptr.is_null() {
                    let status = unsafe { std::ffi::CStr::from_ptr(status_ptr) }.to_string_lossy();
                    if !status.is_empty() && status != "()" {
                        let msg = HarmoniaMessage {
                            id: 0,
                            source: 0,
                            target: 0,
                            kind: ActorKind::Signalograd,
                            timestamp: now_unix(),
                            payload: MessagePayload::StateChanged {
                                to: status.into_owned(),
                            },
                        };
                        let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                    }
                    harmonia_signalograd::harmonia_signalograd_free_string(status_ptr);
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("signalograd", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Signal { payload_sexp } => {
                let c_str = std::ffi::CString::new(payload_sexp).unwrap_or_default();
                harmonia_signalograd::harmonia_signalograd_observe(c_str.as_ptr());
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] SignalogradActor shutting down");
            }
        }
        Ok(())
    }
}

// ── MemoryFieldActor ─────────────────────────────────────────────────

pub struct MemoryFieldActor;

pub struct MemoryFieldState {
    bridge: ActorRef<BridgeMsg>,
    #[allow(dead_code)]
    obs: Option<ActorRef<ObsMsg>>,
    last_basin: String,
}

impl Actor for MemoryFieldActor {
    type Msg = ComponentMsg;
    type State = MemoryFieldState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        harmonia_memory_field::init();
        eprintln!("[INFO] [runtime] MemoryFieldActor started");
        Ok(MemoryFieldState {
            bridge,
            obs,
            last_basin: "thomas-0".into(),
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                // Attractors are stepped by harmonic machine via IPC (:attractor-sync phase).
                // Tick monitors basin transitions and emits StateChanged through bridge.
                if let Ok(status_sexp) = harmonia_memory_field::basin_status() {
                    if let Some(basin) = extract_basin_from_sexp(&status_sexp) {
                        if basin != state.last_basin {
                            // Basin switched — emit StateChanged to bridge for Lisp consumption.
                            let msg = HarmoniaMessage {
                                id: 0,
                                source: 0,
                                target: 0,
                                kind: ActorKind::MemoryField,
                                timestamp: now_unix(),
                                payload: MessagePayload::StateChanged {
                                    to: format!("basin:{}", basin),
                                },
                            };
                            let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                            // Observability trace for basin transition.
                            if let Some(obs) = &state.obs {
                                if harmonia_observability::harmonia_observability_is_standard() {
                                    let obs_opt: Option<ActorRef<ObsMsg>> = Some(obs.clone());
                                    obs_opt.trace_event(
                                        "memory-field-basin-switch",
                                        "chain",
                                        json!({
                                            "from": state.last_basin,
                                            "to": basin
                                        }),
                                    );
                                }
                            }
                            state.last_basin = basin;
                        }
                    }
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("memory-field", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Signal { .. } => {
                // Async field recall trigger from gateway (fire-and-forget).
                // Future: extract concept signature, run field recall, post to bridge.
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] MemoryFieldActor shutting down");
            }
        }
        Ok(())
    }
}

/// Extract basin string from sexp like "(:ok :current :thomas-2 ...)"
fn extract_basin_from_sexp(sexp: &str) -> Option<String> {
    if let Some(pos) = sexp.find(":current ") {
        let rest = &sexp[pos + 9..];
        let end = rest
            .find(|c: char| c.is_whitespace() || c == ')')
            .unwrap_or(rest.len());
        let basin = rest[..end].trim();
        if !basin.is_empty() {
            return Some(basin.to_string());
        }
    }
    None
}

// ── ObservabilityActor ───────────────────────────────────────────────
//
// The single trace sink. All trace data flows as ractor cast (fire-and-forget).
// Owns the sender thread, sampling decisions, and parent→child correlation.

pub struct ObservabilityActor;

/// Maximum dotted_order entries before TTL eviction sweep.
const MAX_DOTTED_ORDERS: usize = 8192;
/// Maximum rejected trace IDs tracked (ring buffer, not clear-all).
const MAX_REJECTED_TRACES: usize = 2048;
/// TTL for dotted_order entries (5 minutes in seconds).
const DOTTED_ORDER_TTL_SECS: u64 = 300;

pub struct ObsActorState {
    pub sender: Option<SyncSender<TraceMessage>>,
    pub config: ObservabilityConfig,
    /// Parent→child dotted_order correlation with timestamps for TTL eviction.
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
                    std::thread::sleep(std::time::Duration::from_millis(50));
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

// ── HarmonicMatrixActor ──────────────────────────────────────────────
//
// Typed message enum — the matrix owns its state through the mailbox.
// All operations are serialized through the actor, no lock contention.

pub enum MatrixMsg {
    RegisterNode {
        id: String,
        kind: String,
    },
    RegisterEdge {
        from: String,
        to: String,
        weight: f64,
        min_harmony: f64,
    },
    ObserveRoute {
        from: String,
        to: String,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    },
    LogEvent {
        component: String,
        direction: String,
        channel: String,
        payload: String,
        success: bool,
        error: String,
    },
    SetToolEnabled {
        node: String,
        enabled: bool,
    },
    RouteAllowed {
        from: String,
        to: String,
        signal: f64,
        noise: f64,
        reply: RpcReplyPort<bool>,
    },
    Report(RpcReplyPort<String>),
    StoreSummary(RpcReplyPort<String>),
    Tick,
    Shutdown,
}

pub struct HarmonicMatrixActor;

impl Actor for HarmonicMatrixActor {
    type Msg = MatrixMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_harmonic_matrix::runtime::store::init();
        eprintln!("[INFO] [runtime] HarmonicMatrixActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            MatrixMsg::RegisterNode { id, kind } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::register_node(&id, &kind) {
                    eprintln!("[WARN] [matrix] register-node failed: {e}");
                }
            }
            MatrixMsg::RegisterEdge {
                from,
                to,
                weight,
                min_harmony,
            } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::register_edge(
                    &from,
                    &to,
                    weight,
                    min_harmony,
                ) {
                    eprintln!("[WARN] [matrix] register-edge failed: {e}");
                }
            }
            MatrixMsg::ObserveRoute {
                from,
                to,
                success,
                latency_ms,
                cost_usd,
            } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::observe_route(
                    &from, &to, success, latency_ms, cost_usd,
                ) {
                    eprintln!("[WARN] [matrix] observe-route failed: {e}");
                }
            }
            MatrixMsg::LogEvent {
                component,
                direction,
                channel,
                payload,
                success,
                error,
            } => {
                if let Err(e) = harmonia_harmonic_matrix::runtime::ops::log_event(
                    &component, &direction, &channel, &payload, success, &error,
                ) {
                    eprintln!("[WARN] [matrix] log-event failed: {e}");
                }
            }
            MatrixMsg::SetToolEnabled { node, enabled } => {
                if let Err(e) =
                    harmonia_harmonic_matrix::runtime::ops::set_tool_enabled(&node, enabled)
                {
                    eprintln!("[WARN] [matrix] set-tool-enabled failed: {e}");
                }
            }
            MatrixMsg::RouteAllowed {
                from,
                to,
                signal,
                noise,
                reply,
            } => {
                let result = harmonia_harmonic_matrix::runtime::ops::route_allowed(
                    &from, &to, signal, noise,
                );
                let _ = reply.send(result.unwrap_or(false));
            }
            MatrixMsg::Report(reply) => {
                let result = harmonia_harmonic_matrix::runtime::reports::report()
                    .unwrap_or_else(|e| format!("(:error \"{}\")", e));
                let _ = reply.send(result);
            }
            MatrixMsg::StoreSummary(reply) => {
                let result = harmonia_harmonic_matrix::runtime::store::store_summary()
                    .unwrap_or_else(|e| format!("(:error \"{}\")", e));
                let _ = reply.send(result);
            }
            MatrixMsg::Tick => {
                // Epoch advancement — future work
            }
            MatrixMsg::Shutdown => {
                eprintln!("[INFO] [runtime] HarmonicMatrixActor shutting down");
            }
        }
        Ok(())
    }
}

// ── GatewayActor ─────────────────────────────────────────────────────

pub struct GatewayActor;

pub struct GatewayState {
    bridge: ActorRef<BridgeMsg>,
    obs: Option<ActorRef<ObsMsg>>,
}

impl Actor for GatewayActor {
    type Msg = ComponentMsg;
    type State = GatewayState;
    type Arguments = (ActorRef<BridgeMsg>, Option<ActorRef<ObsMsg>>);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (bridge, obs): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] GatewayActor started");
        Ok(GatewayState { bridge, obs })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                // Gateway polling: collect inbound signals from all frontends
                let registry = harmonia_gateway::Registry::new();
                let batch = harmonia_gateway::poll_baseband(&registry);
                if harmonia_observability::harmonia_observability_is_verbose()
                    && !batch.envelopes.is_empty()
                {
                    state.obs.trace_event(
                        "gateway-poll",
                        "tool",
                        json!({"envelopes": batch.envelopes.len()}),
                    );
                }
                for envelope in &batch.envelopes {
                    let msg = HarmoniaMessage {
                        id: 0,
                        source: 0,
                        target: 0,
                        kind: ActorKind::Gateway,
                        timestamp: now_unix(),
                        payload: MessagePayload::InboundSignal {
                            envelope_sexp: envelope.to_sexp(),
                        },
                    };
                    let _ = state.bridge.cast(BridgeMsg::Enqueue { msg });
                }
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("gateway", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Signal { .. } => {
                // Outbound signals handled via direct gateway API from SBCL
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] GatewayActor shutting down");
            }
        }
        Ok(())
    }
}

// ── VaultActor ──────────────────────────────────────────────────────

pub struct VaultActor;

impl Actor for VaultActor {
    type Msg = ComponentMsg;
    type State = Option<ActorRef<ObsMsg>>;
    type Arguments = Option<ActorRef<ObsMsg>>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        obs: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] VaultActor started");
        Ok(obs)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Dispatch(sexp, reply) => {
                if harmonia_observability::harmonia_observability_is_verbose() {
                    // Trace vault access (symbol name only, never the value)
                    let symbol = crate::dispatch::extract_vault_symbol(&sexp);
                    if !symbol.is_empty() {
                        state.trace_event("vault-access", "tool", json!({"symbol": symbol}));
                    }
                }
                let result = crate::dispatch::dispatch("vault", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] VaultActor shutting down");
            }
            ComponentMsg::Tick => { /* vault does not tick */ }
            ComponentMsg::Signal { .. } => { /* vault does not handle signals */ }
        }
        Ok(())
    }
}

// ── ConfigActor ─────────────────────────────────────────────────────

pub struct ConfigActor;

impl Actor for ConfigActor {
    type Msg = ComponentMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] ConfigActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("config", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ConfigActor shutting down");
            }
            ComponentMsg::Tick => { /* config does not tick */ }
            ComponentMsg::Signal { .. } => { /* config does not handle signals */ }
        }
        Ok(())
    }
}

// ── ProviderRouterActor ─────────────────────────────────────────────

pub struct ProviderRouterActor;

impl Actor for ProviderRouterActor {
    type Msg = ComponentMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] ProviderRouterActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("provider-router", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ProviderRouterActor shutting down");
            }
            ComponentMsg::Tick => { /* provider-router does not tick */ }
            ComponentMsg::Signal { .. } => { /* provider-router does not handle signals */ }
        }
        Ok(())
    }
}

// ── ParallelActor ───────────────────────────────────────────────────

pub struct ParallelActor;

impl Actor for ParallelActor {
    type Msg = ComponentMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] ParallelActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("parallel", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ParallelActor shutting down");
            }
            ComponentMsg::Tick => { /* parallel does not tick */ }
            ComponentMsg::Signal { .. } => { /* parallel does not handle signals */ }
        }
        Ok(())
    }
}

// ── RouterActor ────────────────────────────────────────────────────

pub struct RouterActor;

/// Per-tier success/cost statistics — 32 bytes, no heap.
#[derive(Debug, Clone, Copy, Default)]
pub struct TierStats {
    pub requests: u64,
    pub successes: u64,
    pub total_cost_usd: f64,
    pub total_latency_ms: u64,
}

/// Tier index — maps tier name to fixed array slot. No HashMap.
const TIER_AUTO: usize = 0;
const TIER_ECO: usize = 1;
const TIER_PREMIUM: usize = 2;
const TIER_FREE: usize = 3;
const TIER_NAMES: [&str; 4] = ["auto", "eco", "premium", "free"];

fn tier_index(tier: &str) -> usize {
    match tier {
        "eco" => TIER_ECO,
        "premium" => TIER_PREMIUM,
        "free" => TIER_FREE,
        _ => TIER_AUTO,
    }
}

/// Active cascade escalation entry — bounded.
#[derive(Debug)]
pub struct CascadeEntry {
    pub request_id: u64,
    pub tier_idx: u8,
    pub attempt_count: u8,
    pub started_at: u64,
}

/// Compact route history entry — 40 bytes.
/// Stores model/task as truncated fixed-size arrays to avoid heap allocation.
const MODEL_ID_CAP: usize = 48;
const TASK_KIND_CAP: usize = 24;

#[derive(Debug)]
pub struct RouteHistoryEntry {
    model_buf: [u8; MODEL_ID_CAP],
    model_len: u8,
    task_buf: [u8; TASK_KIND_CAP],
    task_len: u8,
    pub tier_idx: u8,
    pub success: bool,
    pub latency_ms: u32,
    pub timestamp: u64,
}

impl RouteHistoryEntry {
    fn new(model_id: &str, task_kind: &str, tier: &str, success: bool, latency_ms: u64) -> Self {
        let mut model_buf = [0u8; MODEL_ID_CAP];
        let model_len = model_id.len().min(MODEL_ID_CAP);
        model_buf[..model_len].copy_from_slice(&model_id.as_bytes()[..model_len]);
        let mut task_buf = [0u8; TASK_KIND_CAP];
        let task_len = task_kind.len().min(TASK_KIND_CAP);
        task_buf[..task_len].copy_from_slice(&task_kind.as_bytes()[..task_len]);
        Self {
            model_buf,
            model_len: model_len as u8,
            task_buf,
            task_len: task_len as u8,
            tier_idx: tier_index(tier) as u8,
            success,
            latency_ms: latency_ms.min(u32::MAX as u64) as u32,
            timestamp: harmonia_actor_protocol::now_unix(),
        }
    }

    fn model_id(&self) -> &str {
        std::str::from_utf8(&self.model_buf[..self.model_len as usize]).unwrap_or("")
    }

    fn task_kind(&self) -> &str {
        std::str::from_utf8(&self.task_buf[..self.task_len as usize]).unwrap_or("")
    }

    fn tier_name(&self) -> &'static str {
        TIER_NAMES
            .get(self.tier_idx as usize)
            .copied()
            .unwrap_or("auto")
    }
}

/// Router state — fixed-size tier stats (128 bytes), bounded history ring.
/// No HashMap, no unbounded Vec.
const HISTORY_CAP: usize = 32;
const CASCADE_CAP: usize = 4;

#[derive(Debug)]
pub struct RouterState {
    pub active_tier_idx: u8,
    pub tier_stats: [TierStats; 4],
    history: [Option<RouteHistoryEntry>; HISTORY_CAP],
    history_write: usize,
    history_count: usize,
    pub cascade_entries: [Option<CascadeEntry>; CASCADE_CAP],
    pub obs: Option<ActorRef<ObsMsg>>,
}

impl Default for RouterState {
    fn default() -> Self {
        Self {
            active_tier_idx: TIER_AUTO as u8,
            tier_stats: [TierStats::default(); 4],
            history: std::array::from_fn(|_| None),
            history_write: 0,
            history_count: 0,
            cascade_entries: std::array::from_fn(|_| None),
            obs: None,
        }
    }
}

impl RouterState {
    fn active_tier_name(&self) -> &'static str {
        TIER_NAMES
            .get(self.active_tier_idx as usize)
            .copied()
            .unwrap_or("auto")
    }

    fn record_feedback(
        &mut self,
        model_id: &str,
        task_kind: &str,
        tier: &str,
        success: bool,
        latency_ms: u64,
        cost_usd: f64,
    ) {
        let idx = tier_index(tier);
        let stats = &mut self.tier_stats[idx];
        stats.requests += 1;
        if success {
            stats.successes += 1;
        }
        stats.total_cost_usd += cost_usd;
        stats.total_latency_ms += latency_ms;

        // Ring buffer: overwrite oldest entry
        self.history[self.history_write] = Some(RouteHistoryEntry::new(
            model_id, task_kind, tier, success, latency_ms,
        ));
        self.history_write = (self.history_write + 1) % HISTORY_CAP;
        if self.history_count < HISTORY_CAP {
            self.history_count += 1;
        }
    }

    fn status_sexp(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(1024);
        let _ = write!(
            out,
            "(:ok :result (:active-tier \"{}\" :history-count {} :tier-stats (",
            self.active_tier_name(),
            self.history_count
        );
        for (i, stats) in self.tier_stats.iter().enumerate() {
            if stats.requests == 0 {
                continue;
            }
            let avg_lat = stats.total_latency_ms / stats.requests;
            let sr = stats.successes as f64 / stats.requests as f64;
            let _ = write!(
                out,
                "(:tier \"{}\" :requests {} :success-rate {:.2} :avg-latency-ms {} :total-cost {:.6})",
                TIER_NAMES[i], stats.requests, sr, avg_lat, stats.total_cost_usd
            );
        }
        out.push_str(") :recent (");
        // Show last 5 from ring buffer
        let mut shown = 0;
        let start = if self.history_count >= HISTORY_CAP {
            (self.history_write + HISTORY_CAP - 1) % HISTORY_CAP
        } else if self.history_count > 0 {
            self.history_count - 1
        } else {
            0
        };
        for offset in 0..5 {
            if offset >= self.history_count {
                break;
            }
            let idx = (start + HISTORY_CAP - offset) % HISTORY_CAP;
            if let Some(r) = &self.history[idx] {
                let _ = write!(
                    out,
                    "(:model \"{}\" :task \"{}\" :tier \"{}\" :success {} :latency-ms {} :timestamp {})",
                    r.model_id(),
                    r.task_kind(),
                    r.tier_name(),
                    if r.success { "t" } else { "nil" },
                    r.latency_ms,
                    r.timestamp
                );
                shown += 1;
            }
        }
        let _ = shown;
        out.push_str(") :cascades (");
        for slot in &self.cascade_entries {
            if let Some(c) = slot {
                let _ = write!(
                    out,
                    "(:request-id {} :tier-idx {} :attempts {} :started-at {})",
                    c.request_id, c.tier_idx, c.attempt_count, c.started_at
                );
            }
        }
        out.push_str(")))");
        out
    }
}

impl Actor for RouterActor {
    type Msg = ComponentMsg;
    type State = RouterState;
    type Arguments = Option<ActorRef<ObsMsg>>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        obs: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let tier_str = harmonia_config_store::get_config("router", "router", "active-tier")
            .ok()
            .flatten()
            .unwrap_or_default();
        let idx = tier_index(&tier_str) as u8;
        eprintln!(
            "[INFO] [runtime] RouterActor started, tier={}",
            TIER_NAMES[idx as usize]
        );
        Ok(RouterState {
            active_tier_idx: idx,
            obs,
            ..Default::default()
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                // Sync tier from config-store — picks up changes from any
                // frontend (TUI, MQTT, WhatsApp, etc.) without needing
                // direct actor messaging from the gateway.
                if let Ok(Some(tier_str)) =
                    harmonia_config_store::get_config("router", "router", "active-tier")
                {
                    state.active_tier_idx = tier_index(&tier_str) as u8;
                }
                // Expire stale cascade entries (>30s)
                let now = harmonia_actor_protocol::now_unix();
                for slot in state.cascade_entries.iter_mut() {
                    if let Some(c) = slot {
                        if now - c.started_at >= 30 {
                            *slot = None;
                        }
                    }
                }
            }
            ComponentMsg::Signal { payload_sexp } => {
                if payload_sexp.contains("tier-changed") {
                    if let Some(tier) = extract_sexp_value(&payload_sexp, "tier") {
                        let old_tier = state.active_tier_name().to_string();
                        state.active_tier_idx = tier_index(&tier) as u8;
                        state.obs.trace_event(
                            "router-tier-changed",
                            "tool",
                            json!({"old": old_tier, "new": tier}),
                        );
                    }
                } else if payload_sexp.contains("route-feedback") {
                    let model = extract_sexp_value(&payload_sexp, "model").unwrap_or_default();
                    let task = extract_sexp_value(&payload_sexp, "task").unwrap_or_default();
                    let tier = extract_sexp_value(&payload_sexp, "tier").unwrap_or_default();
                    let success = payload_sexp.contains(":success t");
                    let latency = extract_sexp_u64(&payload_sexp, "latency-ms").unwrap_or(0);
                    let cost = extract_sexp_f64(&payload_sexp, "cost-usd").unwrap_or(0.0);
                    state.record_feedback(&model, &task, &tier, success, latency, cost);
                    if !success && harmonia_observability::harmonia_observability_is_standard() {
                        state.obs.trace_event("router-cascade-escalate", "tool", json!({"model": model, "reason": "route-feedback-failure", "tier": tier}));
                    }
                }
            }
            ComponentMsg::Dispatch(_sexp, reply) => {
                let result = state.status_sexp();
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] RouterActor shutting down");
            }
        }
        Ok(())
    }
}

// ── Router sexp parsing helpers ──────────────────────────────────────

fn extract_sexp_value(sexp: &str, key: &str) -> Option<String> {
    let pattern = format!(":{}  \"", key);
    let pattern2 = format!(":{} \"", key);
    let start = sexp.find(&pattern2).or_else(|| sexp.find(&pattern))?;
    let after_key = start + pattern2.len();
    let rest = &sexp[after_key..];
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

fn extract_sexp_u64(sexp: &str, key: &str) -> Option<u64> {
    let pattern = format!(":{} ", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}

fn extract_sexp_f64(sexp: &str, key: &str) -> Option<f64> {
    let pattern = format!(":{} ", key);
    let start = sexp.find(&pattern)? + pattern.len();
    let rest = &sexp[start..];
    let end = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.' && c != '-')
        .unwrap_or(rest.len());
    rest[..end].parse().ok()
}
