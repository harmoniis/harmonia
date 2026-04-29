//! RouterActor — model tier routing with feedback statistics.

use ractor::{Actor, ActorProcessingErr, ActorRef};
use serde_json::json;

use harmonia_observability::{ObsMsg, Traceable};

use super::ComponentMsg;

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
        let tier_stats_sexp: String = self.tier_stats.iter().enumerate()
            .filter(|(_, stats)| stats.requests > 0)
            .map(|(i, stats)| {
                let avg_lat = stats.total_latency_ms / stats.requests;
                let sr = stats.successes as f64 / stats.requests as f64;
                format!(
                    "(:tier \"{}\" :requests {} :success-rate {:.2} :avg-latency-ms {} :total-cost {:.6})",
                    TIER_NAMES[i], stats.requests, sr, avg_lat, stats.total_cost_usd
                )
            })
            .collect::<Vec<_>>()
            .join("");

        let start = if self.history_count >= HISTORY_CAP {
            (self.history_write + HISTORY_CAP - 1) % HISTORY_CAP
        } else if self.history_count > 0 {
            self.history_count - 1
        } else {
            0
        };
        let recent_sexp: String = (0..5.min(self.history_count))
            .filter_map(|offset| {
                let idx = (start + HISTORY_CAP - offset) % HISTORY_CAP;
                self.history[idx].as_ref().map(|r| format!(
                    "(:model \"{}\" :task \"{}\" :tier \"{}\" :success {} :latency-ms {} :timestamp {})",
                    r.model_id(),
                    r.task_kind(),
                    r.tier_name(),
                    if r.success { "t" } else { "nil" },
                    r.latency_ms,
                    r.timestamp
                ))
            })
            .collect::<Vec<_>>()
            .join("");

        let cascades_sexp: String = self.cascade_entries.iter()
            .filter_map(|slot| slot.as_ref().map(|c| format!(
                "(:request-id {} :tier-idx {} :attempts {} :started-at {})",
                c.request_id, c.tier_idx, c.attempt_count, c.started_at
            )))
            .collect::<Vec<_>>()
            .join("");

        format!(
            "(:ok :result (:active-tier \"{}\" :history-count {} :tier-stats ({}) :recent ({}) :cascades ({})))",
            self.active_tier_name(),
            self.history_count,
            tier_stats_sexp,
            recent_sexp,
            cascades_sexp
        )
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
            ComponentMsg::Dispatch(sexp, reply) => {
                // Route feedback and tier changes arrive as dispatch commands.
                // Use proper :op extraction instead of string containment checks.
                let op = harmonia_actor_protocol::extract_sexp_string(&sexp, ":op")
                    .unwrap_or_default();
                let result = match op.as_str() {
                    "tier-changed" => {
                        if let Some(tier) = harmonia_actor_protocol::extract_sexp_string(&sexp, ":tier") {
                            let old_tier = state.active_tier_name().to_string();
                            state.active_tier_idx = tier_index(&tier) as u8;
                            state.obs.trace_event(
                                "router-tier-changed",
                                "tool",
                                json!({"old": old_tier, "new": tier}),
                            );
                        }
                        "(:ok)".to_string()
                    }
                    "route-feedback" => {
                        let model = harmonia_actor_protocol::extract_sexp_string(&sexp, ":model")
                            .unwrap_or_default();
                        let task = harmonia_actor_protocol::extract_sexp_string(&sexp, ":task")
                            .unwrap_or_default();
                        let tier = harmonia_actor_protocol::extract_sexp_string(&sexp, ":tier")
                            .unwrap_or_default();
                        let success = harmonia_actor_protocol::extract_sexp_bool(&sexp, ":success")
                            .unwrap_or(false);
                        let latency = harmonia_actor_protocol::extract_sexp_u64(&sexp, ":latency-ms")
                            .unwrap_or(0);
                        let cost = harmonia_actor_protocol::extract_sexp_f64(&sexp, ":cost-usd")
                            .unwrap_or(0.0);
                        state.record_feedback(&model, &task, &tier, success, latency, cost);
                        if !success && harmonia_observability::harmonia_observability_is_standard() {
                            state.obs.trace_event("router-cascade-escalate", "tool", json!({"model": model, "reason": "route-feedback-failure", "tier": tier}));
                        }
                        "(:ok)".to_string()
                    }
                    "status" | _ => state.status_sexp(),
                };
                let _ = reply.send(result);
            }
            ComponentMsg::Shutdown => {
                eprintln!("[INFO] [runtime] RouterActor shutting down");
            }
        }
        Ok(())
    }
}

