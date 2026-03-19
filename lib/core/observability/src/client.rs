//! LangSmith REST client.
//!
//! Minimal client for the LangSmith tracing API:
//! - POST /runs/batch — batch create/update runs

use crate::model::TraceSpan;
use serde_json::{json, Value};

pub struct LangSmithClient {
    api_url: String,
    api_key: String,
    agent: ureq::Agent,
}

/// Flush result — distinguishes rate limits from other errors.
pub enum FlushResult {
    Ok,
    RateLimited(String), // response body — may contain plan/quota details
    Error(String),
}

impl LangSmithClient {
    pub fn new(api_url: &str, api_key: &str) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build();
        Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            api_key: api_key.to_string(),
            agent,
        }
    }

    /// Batch-submit run creates and updates.
    pub fn post_runs_batch(&self, creates: &[Value], updates: &[Value]) -> FlushResult {
        if creates.is_empty() && updates.is_empty() {
            return FlushResult::Ok;
        }

        let body = json!({
            "post": creates,
            "patch": updates,
        });

        let url = format!("{}/runs/batch", self.api_url);
        match self
            .agent
            .post(&url)
            .set("x-api-key", &self.api_key)
            .set("Content-Type", "application/json")
            .send_json(body)
        {
            Ok(_) => FlushResult::Ok,
            Err(ureq::Error::Status(code, response)) => {
                let body = response.into_string().unwrap_or_default();
                if code == 429 {
                    FlushResult::RateLimited(body)
                } else {
                    FlushResult::Error(format!("status {code}: {body}"))
                }
            }
            Err(e) => FlushResult::Error(format!("{e}")),
        }
    }

    /// Convert a TraceSpan to a LangSmith run create payload.
    pub fn span_to_create(span: &TraceSpan) -> Value {
        let mut run = json!({
            "id": span.run_id,
            "name": span.name,
            "run_type": span.run_type,
            "start_time": span.start_time,
            "inputs": span.inputs,
            "extra": span.extra,
            "trace_id": span.trace_id,
            "dotted_order": span.dotted_order,
            "session_name": span.project_name,
        });

        if let Some(ref parent) = span.parent_run_id {
            run["parent_run_id"] = json!(parent);
        }
        if let Some(ref end) = span.end_time {
            run["end_time"] = json!(end);
        }
        if let Some(ref outputs) = span.outputs {
            run["outputs"] = outputs.clone();
        }
        if let Some(ref status) = span.status {
            run["status"] = json!(status);
        }

        run
    }

    /// Build a run update (patch) payload.
    pub fn build_update(run_id: &str, status: &str, outputs: &Value, end_time: &str) -> Value {
        json!({
            "id": run_id,
            "status": status,
            "outputs": outputs,
            "end_time": end_time,
            "trace_id": run_id,
        })
    }
}
