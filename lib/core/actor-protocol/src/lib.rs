//! Unified actor protocol types for the Harmonia agent.
//!
//! This crate provides **types only** — no global state, no statics, no FFI.
//! The actor registry now lives in `harmonia-runtime` as a ractor actor.
//! SBCL communicates via IPC (Unix domain socket) instead of dlsym.

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
    Router,
    MemoryField,
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
            ActorKind::Router => "router",
            ActorKind::MemoryField => "memory-field",
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
            "router" => Ok(ActorKind::Router),
            "memory-field" => Ok(ActorKind::MemoryField),
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
    /// User changed routing tier via /auto /eco /premium /free.
    TierChanged {
        tier: String,
    },
    /// Feedback from a completed LLM route for experience tracking.
    RouteFeedback {
        request_id: u64,
        model_id: String,
        task_kind: String,
        tier: String,
        success: bool,
        latency_ms: u64,
        cost_usd_estimate: f64,
        complexity_score: f64,
    },
    /// Cascade escalation: a model failed, try next in chain.
    CascadeEscalate {
        request_id: u64,
        failed_model: String,
        reason: String,
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
            MessagePayload::TierChanged { tier } => {
                format!(":tier-changed :tier \"{}\"", sexp_escape(tier))
            }
            MessagePayload::RouteFeedback {
                request_id,
                model_id,
                task_kind,
                tier,
                success,
                latency_ms,
                cost_usd_estimate,
                complexity_score,
            } => format!(
                ":route-feedback :request-id {} :model \"{}\" :task \"{}\" :tier \"{}\" :success {} :latency-ms {} :cost-usd {:.6} :complexity {:.4}",
                request_id,
                sexp_escape(model_id),
                sexp_escape(task_kind),
                sexp_escape(tier),
                if *success { "t" } else { "nil" },
                latency_ms,
                cost_usd_estimate,
                complexity_score
            ),
            MessagePayload::CascadeEscalate {
                request_id,
                failed_model,
                reason,
            } => format!(
                ":cascade-escalate :request-id {} :failed-model \"{}\" :reason \"{}\"",
                request_id,
                sexp_escape(failed_model),
                sexp_escape(reason)
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

    /// Append sexp representation to an existing buffer (zero intermediate allocation).
    pub fn write_sexp(&self, buf: &mut String) {
        use std::fmt::Write;
        let _ = write!(buf, "(:actor-id {} :kind :{} :timestamp {} :payload (",
            self.source, self.kind.as_str(), self.timestamp);
        match &self.payload {
            MessagePayload::InboundSignal { envelope_sexp } => {
                let _ = write!(buf, ":inbound-signal :envelope \"{}\"", sexp_escape(envelope_sexp));
            }
            MessagePayload::OutboundSignal { frontend, sub_channel, payload } => {
                let _ = write!(buf, ":outbound-signal :frontend \"{}\" :sub-channel \"{}\" :payload \"{}\"",
                    sexp_escape(frontend), sexp_escape(sub_channel), sexp_escape(payload));
            }
            MessagePayload::TaskCompleted { output, exit_code, duration_ms } => {
                let _ = write!(buf, ":completed :output \"{}\" :exit-code {} :duration-ms {}",
                    sexp_escape(output), exit_code, duration_ms);
            }
            MessagePayload::TaskFailed { error, duration_ms } => {
                let _ = write!(buf, ":failed :error \"{}\" :duration-ms {}",
                    sexp_escape(error), duration_ms);
            }
            MessagePayload::ProgressHeartbeat { bytes_delta } => {
                let _ = write!(buf, ":progress-heartbeat :bytes-delta {}", bytes_delta);
            }
            MessagePayload::StateChanged { to } => {
                let _ = write!(buf, ":state-changed :to {}", to);
            }
            MessagePayload::MeshInbound { from_node, msg_type, payload } => {
                let _ = write!(buf, ":mesh-inbound :from-node \"{}\" :msg-type \"{}\" :payload \"{}\"",
                    sexp_escape(from_node), sexp_escape(msg_type), sexp_escape(payload));
            }
            MessagePayload::RecordAck { table, count } => {
                let _ = write!(buf, ":record-ack :table \"{}\" :count {}", sexp_escape(table), count);
            }
            MessagePayload::ToolInvoked { tool_name, operation, request_id } => {
                let _ = write!(buf, ":tool-invoked :tool \"{}\" :operation \"{}\" :request-id {}",
                    sexp_escape(tool_name), sexp_escape(operation), request_id);
            }
            MessagePayload::ToolCompleted { tool_name, operation, request_id, envelope_sexp, duration_ms } => {
                let _ = write!(buf, ":tool-completed :tool \"{}\" :operation \"{}\" :request-id {} :envelope \"{}\" :duration-ms {}",
                    sexp_escape(tool_name), sexp_escape(operation), request_id, sexp_escape(envelope_sexp), duration_ms);
            }
            MessagePayload::ToolFailed { tool_name, operation, request_id, error, duration_ms } => {
                let _ = write!(buf, ":tool-failed :tool \"{}\" :operation \"{}\" :request-id {} :error \"{}\" :duration-ms {}",
                    sexp_escape(tool_name), sexp_escape(operation), request_id, sexp_escape(error), duration_ms);
            }
            MessagePayload::Shutdown => { buf.push_str(":shutdown"); }
            MessagePayload::SupervisionReady { task, spec, taxonomy, assertions } => {
                let _ = write!(buf, ":supervision-ready :task {} :spec {} :taxonomy \"{}\" :assertions {}",
                    task, spec, sexp_escape(taxonomy), assertions);
            }
            MessagePayload::SupervisionVerdict { task, spec, passed, failed, skipped, confidence, grade, summary } => {
                let _ = write!(buf, ":supervision-verdict :task {} :spec {} :passed {} :failed {} :skipped {} :confidence {:.4} :grade \"{}\" :summary \"{}\"",
                    task, spec, passed, failed, skipped, confidence, sexp_escape(grade), sexp_escape(summary));
            }
            MessagePayload::TierChanged { tier } => {
                let _ = write!(buf, ":tier-changed :tier \"{}\"", sexp_escape(tier));
            }
            MessagePayload::RouteFeedback { request_id, model_id, task_kind, tier, success, latency_ms, cost_usd_estimate, complexity_score } => {
                let _ = write!(buf, ":route-feedback :request-id {} :model \"{}\" :task \"{}\" :tier \"{}\" :success {} :latency-ms {} :cost-usd {:.6} :complexity {:.4}",
                    request_id, sexp_escape(model_id), sexp_escape(task_kind), sexp_escape(tier),
                    if *success { "t" } else { "nil" }, latency_ms, cost_usd_estimate, complexity_score);
            }
            MessagePayload::CascadeEscalate { request_id, failed_model, reason } => {
                let _ = write!(buf, ":cascade-escalate :request-id {} :failed-model \"{}\" :reason \"{}\"",
                    request_id, sexp_escape(failed_model), sexp_escape(reason));
            }
        }
        buf.push_str("))");
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

// ActorRegistry has been removed — it now lives in harmonia-runtime
// as the RuntimeSupervisor actor (lib/core/runtime/src/supervisor.rs).

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

/// Extract a quoted string value from a sexp after the given key.
/// Handles escaped quotes (\") and backslashes (\\) correctly.
pub fn extract_sexp_string(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = &sexp[idx + key.len()..];
    let after = after.trim_start();
    if !after.starts_with('"') {
        // Unquoted value: take until whitespace or closing paren
        let val: String = after.chars().take_while(|c| !c.is_whitespace() && *c != ')').collect();
        return if val.is_empty() { None } else { Some(val) };
    }
    let inner = &after[1..];
    let bytes = inner.as_bytes();
    let mut result = String::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'"' {
            return Some(result);
        }
        if bytes[i] == b'\\' && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'"' => result.push('"'),
                b'\\' => result.push('\\'),
                b'n' => result.push('\n'),
                b'r' => result.push('\r'),
                b't' => result.push('\t'),
                other => { result.push('\\'); result.push(other as char); }
            }
            i += 2;
        } else {
            result.push(bytes[i] as char);
            i += 1;
        }
    }
    None // Unclosed quote
}

// ─── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

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
