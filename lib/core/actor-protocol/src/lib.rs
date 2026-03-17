//! Unified actor protocol for the Harmonia agent.
//!
//! This crate provides **types only** — no global state, no statics.
//! The single `ActorRegistry` instance lives in `harmonia-parallel-agents`
//! (the `actor_core` module), which exports the FFI functions.
//!
//! Other crates (gateway, tailnet, chronicle) use the `client` module
//! to call those FFI functions via `dlsym(RTLD_DEFAULT, ...)` at runtime,
//! so all crates share the ONE registry owned by parallel-agents.

pub mod client;

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

// ─── Actor identity ─────────────────────────────────────────────────────

pub type ActorId = u64;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ActorKind {
    Gateway,
    CliAgent,
    LlmTask,
    Chronicle,
    Tailnet,
    Signalograd,
    Tool,
    Supervisor,
    Observability,
}

impl ActorKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ActorKind::Gateway => "gateway",
            ActorKind::CliAgent => "cli-agent",
            ActorKind::LlmTask => "llm-task",
            ActorKind::Chronicle => "chronicle",
            ActorKind::Tailnet => "tailnet",
            ActorKind::Signalograd => "signalograd",
            ActorKind::Tool => "tool",
            ActorKind::Supervisor => "supervisor",
            ActorKind::Observability => "observability",
        }
    }

    pub fn from_str(s: &str) -> Result<Self, String> {
        match s {
            "gateway" => Ok(ActorKind::Gateway),
            "cli-agent" => Ok(ActorKind::CliAgent),
            "llm-task" => Ok(ActorKind::LlmTask),
            "chronicle" => Ok(ActorKind::Chronicle),
            "tailnet" => Ok(ActorKind::Tailnet),
            "signalograd" => Ok(ActorKind::Signalograd),
            "tool" => Ok(ActorKind::Tool),
            "supervisor" => Ok(ActorKind::Supervisor),
            "observability" => Ok(ActorKind::Observability),
            _ => Err(format!("unknown actor kind: {}", s)),
        }
    }
}

// ─── Actor state ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq)]
pub enum ActorState {
    Starting,
    Running,
    Idle,
    Completed,
    Failed(String),
    Terminated,
}

impl ActorState {
    pub fn as_str(&self) -> &str {
        match self {
            ActorState::Starting => "starting",
            ActorState::Running => "running",
            ActorState::Idle => "idle",
            ActorState::Completed => "completed",
            ActorState::Failed(_) => "failed",
            ActorState::Terminated => "terminated",
        }
    }
}

// ─── Message protocol ───────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct HarmoniaMessage {
    pub id: u64,
    pub source: ActorId, // 0 = system/external
    pub target: ActorId, // 0 = supervisor
    pub kind: ActorKind, // source actor kind (for Lisp dispatch)
    pub timestamp: u64,
    pub payload: MessagePayload,
}

#[derive(Clone, Debug)]
pub enum MessagePayload {
    InboundSignal {
        envelope_sexp: String,
    },
    OutboundSignal {
        frontend: String,
        sub_channel: String,
        payload: String,
    },
    TaskCompleted {
        output: String,
        exit_code: i32,
        duration_ms: u64,
    },
    TaskFailed {
        error: String,
        duration_ms: u64,
    },
    ProgressHeartbeat {
        bytes_delta: u64,
    },
    StateChanged {
        to: String,
    },
    MeshInbound {
        from_node: String,
        msg_type: String,
        payload: String,
    },
    RecordAck {
        table: String,
        count: u64,
    },
    ToolInvoked {
        tool_name: String,
        operation: String,
        request_id: u64,
    },
    ToolCompleted {
        tool_name: String,
        operation: String,
        request_id: u64,
        envelope_sexp: String,
        duration_ms: u64,
    },
    ToolFailed {
        tool_name: String,
        operation: String,
        request_id: u64,
        error: String,
        duration_ms: u64,
    },
    Shutdown,
    SupervisionReady {
        task: u64,
        spec: u64,
        taxonomy: String,
        assertions: u32,
    },
    SupervisionVerdict {
        task: u64,
        spec: u64,
        passed: u32,
        failed: u32,
        skipped: u32,
        confidence: f64,
        grade: String,
        summary: String,
    },
}

impl HarmoniaMessage {
    pub fn to_sexp(&self) -> String {
        let payload_sexp = match &self.payload {
            MessagePayload::InboundSignal { envelope_sexp } => {
                format!(
                    ":inbound-signal :envelope \"{}\"",
                    sexp_escape(envelope_sexp)
                )
            }
            MessagePayload::OutboundSignal {
                frontend,
                sub_channel,
                payload,
            } => format!(
                ":outbound-signal :frontend \"{}\" :sub-channel \"{}\" :payload \"{}\"",
                sexp_escape(frontend),
                sexp_escape(sub_channel),
                sexp_escape(payload)
            ),
            MessagePayload::TaskCompleted {
                output,
                exit_code,
                duration_ms,
            } => format!(
                ":completed :output \"{}\" :exit-code {} :duration-ms {}",
                sexp_escape(output),
                exit_code,
                duration_ms
            ),
            MessagePayload::TaskFailed { error, duration_ms } => format!(
                ":failed :error \"{}\" :duration-ms {}",
                sexp_escape(error),
                duration_ms
            ),
            MessagePayload::ProgressHeartbeat { bytes_delta } => {
                format!(":progress-heartbeat :bytes-delta {}", bytes_delta)
            }
            MessagePayload::StateChanged { to } => {
                format!(":state-changed :to {}", to)
            }
            MessagePayload::MeshInbound {
                from_node,
                msg_type,
                payload,
            } => format!(
                ":mesh-inbound :from-node \"{}\" :msg-type \"{}\" :payload \"{}\"",
                sexp_escape(from_node),
                sexp_escape(msg_type),
                sexp_escape(payload)
            ),
            MessagePayload::RecordAck { table, count } => {
                format!(
                    ":record-ack :table \"{}\" :count {}",
                    sexp_escape(table),
                    count
                )
            }
            MessagePayload::ToolInvoked {
                tool_name,
                operation,
                request_id,
            } => format!(
                ":tool-invoked :tool \"{}\" :operation \"{}\" :request-id {}",
                sexp_escape(tool_name),
                sexp_escape(operation),
                request_id
            ),
            MessagePayload::ToolCompleted {
                tool_name,
                operation,
                request_id,
                envelope_sexp,
                duration_ms,
            } => format!(
                ":tool-completed :tool \"{}\" :operation \"{}\" :request-id {} :envelope \"{}\" :duration-ms {}",
                sexp_escape(tool_name),
                sexp_escape(operation),
                request_id,
                sexp_escape(envelope_sexp),
                duration_ms
            ),
            MessagePayload::ToolFailed {
                tool_name,
                operation,
                request_id,
                error,
                duration_ms,
            } => format!(
                ":tool-failed :tool \"{}\" :operation \"{}\" :request-id {} :error \"{}\" :duration-ms {}",
                sexp_escape(tool_name),
                sexp_escape(operation),
                request_id,
                sexp_escape(error),
                duration_ms
            ),
            MessagePayload::Shutdown => ":shutdown".to_string(),
            MessagePayload::SupervisionReady {
                task,
                spec,
                taxonomy,
                assertions,
            } => format!(
                ":supervision-ready :task {} :spec {} :taxonomy \"{}\" :assertions {}",
                task,
                spec,
                sexp_escape(taxonomy),
                assertions
            ),
            MessagePayload::SupervisionVerdict {
                task,
                spec,
                passed,
                failed,
                skipped,
                confidence,
                grade,
                summary,
            } => format!(
                ":supervision-verdict :task {} :spec {} :passed {} :failed {} :skipped {} :confidence {:.4} :grade \"{}\" :summary \"{}\"",
                task,
                spec,
                passed,
                failed,
                skipped,
                confidence,
                sexp_escape(grade),
                sexp_escape(summary)
            ),
        };
        format!(
            "(:actor-id {} :kind :{} :timestamp {} :payload ({}))",
            self.source,
            self.kind.as_str(),
            self.timestamp,
            payload_sexp
        )
    }
}

// ─── Actor registration ────────────────────────────────────────────────

pub struct ActorRegistration {
    pub id: ActorId,
    pub kind: ActorKind,
    pub state: ActorState,
    pub registered_at: u64,
    pub last_heartbeat: u64,
    pub stall_ticks: u32,
    pub message_count: u64,
}

pub struct ActorRegistry {
    next_id: u64,
    pub actors: HashMap<ActorId, ActorRegistration>,
    pub mailbox: VecDeque<HarmoniaMessage>,
    next_msg_id: u64,
}

impl ActorRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            actors: HashMap::new(),
            mailbox: VecDeque::new(),
            next_msg_id: 1,
        }
    }

    pub fn register(&mut self, kind: ActorKind) -> ActorId {
        let id = self.next_id;
        self.next_id += 1;
        let now = now_unix();
        self.actors.insert(
            id,
            ActorRegistration {
                id,
                kind,
                state: ActorState::Starting,
                registered_at: now,
                last_heartbeat: now,
                stall_ticks: 0,
                message_count: 0,
            },
        );
        id
    }

    pub fn deregister(&mut self, id: ActorId) -> bool {
        self.actors.remove(&id).is_some()
    }

    pub fn heartbeat(&mut self, id: ActorId, bytes_delta: u64) -> bool {
        let kind = if let Some(reg) = self.actors.get_mut(&id) {
            reg.last_heartbeat = now_unix();
            reg.stall_ticks = 0;
            reg.state = ActorState::Running;
            if bytes_delta > 0 {
                Some(reg.kind.clone())
            } else {
                None
            }
        } else {
            return false;
        };
        if let Some(kind) = kind {
            let msg_id = self.next_msg_id();
            self.post(HarmoniaMessage {
                id: msg_id,
                source: id,
                target: 0,
                kind,
                timestamp: now_unix(),
                payload: MessagePayload::ProgressHeartbeat { bytes_delta },
            });
        }
        true
    }

    pub fn post(&mut self, msg: HarmoniaMessage) {
        if let Some(reg) = self.actors.get_mut(&msg.source) {
            reg.message_count += 1;
        }
        self.mailbox.push_back(msg);
    }

    pub fn post_from(
        &mut self,
        source: ActorId,
        target: ActorId,
        kind: ActorKind,
        payload: MessagePayload,
    ) {
        let msg = HarmoniaMessage {
            id: self.next_msg_id(),
            source,
            target,
            kind,
            timestamp: now_unix(),
            payload,
        };
        self.post(msg);
    }

    pub fn drain(&mut self) -> Vec<HarmoniaMessage> {
        self.mailbox.drain(..).collect()
    }

    pub fn drain_sexp(&mut self) -> String {
        if self.mailbox.is_empty() {
            return "()".to_string();
        }
        let messages: Vec<String> = self.mailbox.drain(..).map(|m| m.to_sexp()).collect();
        format!("({})", messages.join(" "))
    }

    pub fn set_state(&mut self, id: ActorId, state: ActorState) {
        if let Some(reg) = self.actors.get_mut(&id) {
            reg.state = state;
        }
    }

    pub fn actor_state_sexp(&self, id: ActorId) -> String {
        match self.actors.get(&id) {
            Some(reg) => format!(
                "(:id {} :kind :{} :state :{} :registered-at {} :last-heartbeat {} :stall-ticks {} :message-count {})",
                reg.id,
                reg.kind.as_str(),
                reg.state.as_str(),
                reg.registered_at,
                reg.last_heartbeat,
                reg.stall_ticks,
                reg.message_count,
            ),
            None => format!("(:error \"actor {} not found\")", id),
        }
    }

    pub fn list_sexp(&self) -> String {
        if self.actors.is_empty() {
            return "()".to_string();
        }
        let mut entries: Vec<String> = self
            .actors
            .values()
            .map(|reg| {
                format!(
                    "(:id {} :kind :{} :state :{})",
                    reg.id,
                    reg.kind.as_str(),
                    reg.state.as_str()
                )
            })
            .collect();
        entries.sort();
        format!("({})", entries.join(" "))
    }

    fn next_msg_id(&mut self) -> u64 {
        let id = self.next_msg_id;
        self.next_msg_id += 1;
        id
    }
}

// ─── Helpers ────────────────────────────────────────────────────────────

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn sexp_escape(input: &str) -> String {
    // CL's reader handles literal newlines inside strings natively.
    // Only backslash and double-quote need escaping — \n in a CL string
    // literal means literal 'n', NOT a newline, so we must NOT escape them.
    input.replace('\\', "\\\\").replace('"', "\\\"")
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_and_list() {
        let mut reg = ActorRegistry::new();
        let id1 = reg.register(ActorKind::Gateway);
        let id2 = reg.register(ActorKind::CliAgent);
        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(reg.actors.len(), 2);
        let sexp = reg.list_sexp();
        assert!(sexp.contains(":gateway"));
        assert!(sexp.contains(":cli-agent"));
    }

    #[test]
    fn post_and_drain() {
        let mut reg = ActorRegistry::new();
        let id = reg.register(ActorKind::CliAgent);
        reg.post_from(
            id,
            0,
            ActorKind::CliAgent,
            MessagePayload::TaskCompleted {
                output: "hello".to_string(),
                exit_code: 0,
                duration_ms: 100,
            },
        );
        reg.post_from(
            id,
            0,
            ActorKind::CliAgent,
            MessagePayload::ProgressHeartbeat { bytes_delta: 42 },
        );
        assert_eq!(reg.mailbox.len(), 2);
        let drained = reg.drain_sexp();
        assert!(drained.contains(":completed"));
        assert!(drained.contains(":progress-heartbeat"));
        assert!(reg.mailbox.is_empty());
    }

    #[test]
    fn deregister() {
        let mut reg = ActorRegistry::new();
        let id = reg.register(ActorKind::Tailnet);
        assert!(reg.deregister(id));
        assert!(!reg.deregister(id)); // already removed
        assert!(reg.actors.is_empty());
    }

    #[test]
    fn heartbeat_updates_state() {
        let mut reg = ActorRegistry::new();
        let id = reg.register(ActorKind::Gateway);
        assert_eq!(reg.actors[&id].state, ActorState::Starting);
        reg.heartbeat(id, 100);
        assert_eq!(reg.actors[&id].state, ActorState::Running);
        assert_eq!(reg.actors[&id].stall_ticks, 0);
    }

    #[test]
    fn drain_empty_returns_parens() {
        let mut reg = ActorRegistry::new();
        assert_eq!(reg.drain_sexp(), "()");
    }

    #[test]
    fn message_sexp_format() {
        let msg = HarmoniaMessage {
            id: 1,
            source: 5,
            target: 0,
            kind: ActorKind::Gateway,
            timestamp: 1234567890,
            payload: MessagePayload::InboundSignal {
                envelope_sexp: "(:test t)".to_string(),
            },
        };
        let sexp = msg.to_sexp();
        assert!(sexp.contains(":actor-id 5"));
        assert!(sexp.contains(":kind :gateway"));
        assert!(sexp.contains(":inbound-signal"));
    }

    #[test]
    fn mesh_inbound_sexp() {
        let msg = HarmoniaMessage {
            id: 1,
            source: 3,
            target: 0,
            kind: ActorKind::Tailnet,
            timestamp: 100,
            payload: MessagePayload::MeshInbound {
                from_node: "node-1".to_string(),
                msg_type: "relay".to_string(),
                payload: "hello".to_string(),
            },
        };
        let sexp = msg.to_sexp();
        assert!(sexp.contains(":mesh-inbound"));
        assert!(sexp.contains(":from-node \"node-1\""));
    }

    #[test]
    fn actor_kind_roundtrip() {
        for kind in &[
            ActorKind::Gateway,
            ActorKind::CliAgent,
            ActorKind::LlmTask,
            ActorKind::Chronicle,
            ActorKind::Tailnet,
            ActorKind::Signalograd,
            ActorKind::Tool,
            ActorKind::Supervisor,
            ActorKind::Observability,
        ] {
            let s = kind.as_str();
            let parsed = ActorKind::from_str(s).unwrap();
            assert_eq!(&parsed, kind);
        }
    }
}
