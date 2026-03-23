//! Ergonomic trace API for actors.
//!
//! The `Traceable` trait provides a zero-cost abstraction for actors that
//! hold an `Option<ActorRef<ObsMsg>>`. When `obs` is `None`, every method
//! compiles down to a single pointer check — no allocation, no work.

use ractor::ActorRef;
use serde_json::Value;

use crate::model::ObsMsg;

/// Ergonomic trace API for actors holding an `Option<ActorRef<ObsMsg>>`.
///
/// Usage: `state.obs.trace_event("gateway-poll", "tool", json!({"n": count}))`
pub trait Traceable {
    fn trace_event(&self, name: &str, run_type: &str, metadata: Value);
    fn trace_span_start(&self, name: &str, run_type: &str, meta: Value) -> String;
    fn trace_span_end(&self, run_id: &str, status: &str, outputs: Value);
}

impl Traceable for Option<ActorRef<ObsMsg>> {
    fn trace_event(&self, name: &str, run_type: &str, metadata: Value) {
        if let Some(obs) = self {
            let _ = obs.cast(ObsMsg::Event {
                name: name.into(),
                run_type: run_type.into(),
                metadata,
                parent_run_id: None,
                trace_id: None,
            });
        }
    }

    fn trace_span_start(&self, name: &str, run_type: &str, meta: Value) -> String {
        let run_id = uuid::Uuid::new_v4().to_string();
        if let Some(obs) = self {
            let _ = obs.cast(ObsMsg::SpanStart {
                run_id: run_id.clone(),
                parent_run_id: None,
                trace_id: None,
                name: name.into(),
                run_type: run_type.into(),
                metadata: meta,
            });
        }
        run_id
    }

    fn trace_span_end(&self, run_id: &str, status: &str, outputs: Value) {
        if let Some(obs) = self {
            let _ = obs.cast(ObsMsg::SpanEnd {
                run_id: run_id.into(),
                status: status.into(),
                outputs,
            });
        }
    }
}
