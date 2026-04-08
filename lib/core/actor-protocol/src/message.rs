//! HarmoniaMessage: the wire type for all IPC messages, plus sexp serialization.

use std::time::{SystemTime, UNIX_EPOCH};

use crate::actor::{ActorId, ActorKind};
use crate::payload::MessagePayload;
use crate::sexp::escape as sexp_escape;

// ---- Message protocol ------------------------------------------------------

#[derive(Clone, Debug)]
pub struct HarmoniaMessage {
    pub id: u64,
    pub source: ActorId, // 0 = system/external
    pub target: ActorId, // 0 = supervisor
    pub kind: ActorKind, // source actor kind (for Lisp dispatch)
    pub timestamp: u64,
    pub payload: MessagePayload,
}

impl HarmoniaMessage {
    pub fn to_sexp(&self) -> String {
        let payload_sexp = payload_to_sexp(&self.payload);
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
        write_payload_sexp(buf, &self.payload);
        buf.push_str("))");
    }
}

// ---- Helpers ---------------------------------------------------------------

pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ---- Sexp serialization (private) ------------------------------------------

fn payload_to_sexp(payload: &MessagePayload) -> String {
    match payload {
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
    }
}

fn write_payload_sexp(buf: &mut String, payload: &MessagePayload) {
    use std::fmt::Write;
    match payload {
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
}

// ---- Tests -----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::ActorKind;

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
