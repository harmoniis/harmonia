//! Actor wrappers for Harmonia components.
//!
//! Each wrapper is a ractor actor that owns a component's lifecycle.
//! pre_start initializes the component, handle() dispatches messages
//! to the component's public API, and supervision handles recovery.

use ractor::{Actor, ActorProcessingErr, ActorRef};

use harmonia_actor_protocol::{now_unix, ActorKind, HarmoniaMessage, MessagePayload};

use crate::msg::BridgeMsg;

// ── Shared message type for all component actors ─────────────────────

use ractor::RpcReplyPort;

#[allow(dead_code)]
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
                let _ = harmonia_chronicle::gc();
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("chronicle", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] ChronicleActor shutting down");
            }
            _ => {}
        }
        Ok(())
    }
}

// ── TailnetActor ─────────────────────────────────────────────────────

pub struct TailnetActor;

pub struct TailnetState {
    bridge: ActorRef<BridgeMsg>,
}

impl Actor for TailnetActor {
    type Msg = ComponentMsg;
    type State = TailnetState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        let _ = harmonia_tailnet::transport::start_listener();
        eprintln!("[INFO] [runtime] TailnetActor started");
        Ok(TailnetState { bridge })
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
}

impl Actor for SignalogradActor {
    type Msg = ComponentMsg;
    type State = SignalogradState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        harmonia_signalograd::harmonia_signalograd_init();
        eprintln!("[INFO] [runtime] SignalogradActor started");
        Ok(SignalogradState { bridge })
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

// ── ObservabilityActor ───────────────────────────────────────────────

pub struct ObservabilityActor;

impl Actor for ObservabilityActor {
    type Msg = ComponentMsg;
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        // Init is handled by Lisp via IPC dispatch ("observability" "init").
        // The actor only owns flush/shutdown lifecycle.
        eprintln!("[INFO] [runtime] ObservabilityActor started");
        Ok(())
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        message: Self::Msg,
        _state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match message {
            ComponentMsg::Tick => {
                // No-op: the background sender thread auto-flushes every 2s
                // when items are pending. Forcing flush here on every tick (5s)
                // causes redundant HTTP POSTs that trigger 429 rate limits.
            }
            ComponentMsg::Dispatch(sexp, reply) => {
                let result = crate::dispatch::dispatch("observability", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Signal { payload_sexp } => {
                let _ = payload_sexp;
            }
            ComponentMsg::Shutdown => {
                // Flush remaining traces before shutdown, then stop sender thread.
                harmonia_observability::harmonia_observability_flush();
                harmonia_observability::harmonia_observability_shutdown();
                eprintln!("[INFO] [runtime] ObservabilityActor shutting down");
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
}

impl Actor for GatewayActor {
    type Msg = ComponentMsg;
    type State = GatewayState;
    type Arguments = ActorRef<BridgeMsg>;

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        bridge: ActorRef<BridgeMsg>,
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] GatewayActor started");
        Ok(GatewayState { bridge })
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
    type State = ();
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
    ) -> Result<Self::State, ActorProcessingErr> {
        eprintln!("[INFO] [runtime] VaultActor started");
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
                let result = crate::dispatch::dispatch("vault", &sexp);
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] VaultActor shutting down");
            }
            _ => {}
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
            _ => {}
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
            _ => {}
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
            _ => {}
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
                    "(:model \"{}\" :task \"{}\" :tier \"{}\" :success {} :latency-ms {})",
                    r.model_id(),
                    r.task_kind(),
                    r.tier_name(),
                    if r.success { "t" } else { "nil" },
                    r.latency_ms
                );
                shown += 1;
            }
        }
        let _ = shown;
        out.push_str(")))");
        out
    }
}

impl Actor for RouterActor {
    type Msg = ComponentMsg;
    type State = RouterState;
    type Arguments = ();

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        _args: (),
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
                        state.active_tier_idx = tier_index(&tier) as u8;
                    }
                } else if payload_sexp.contains("route-feedback") {
                    let model = extract_sexp_value(&payload_sexp, "model").unwrap_or_default();
                    let task = extract_sexp_value(&payload_sexp, "task").unwrap_or_default();
                    let tier = extract_sexp_value(&payload_sexp, "tier").unwrap_or_default();
                    let success = payload_sexp.contains(":success t");
                    let latency = extract_sexp_u64(&payload_sexp, "latency-ms").unwrap_or(0);
                    let cost = extract_sexp_f64(&payload_sexp, "cost-usd").unwrap_or(0.0);
                    state.record_feedback(&model, &task, &tier, success, latency, cost);
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
