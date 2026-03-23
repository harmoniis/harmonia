//! LangSmith trace provider.

use crate::backend::{FlushResult, TraceBackend};
use crate::config::ObservabilityConfig;
use crate::model::TraceSpan;
use serde_json::{json, Value};

pub struct LangSmith {
    api_url: String,
    api_key: String,
    agent: ureq::Agent,
}

impl LangSmith {
    /// Create from config. Returns None if api_key is missing.
    pub fn from_config(config: &ObservabilityConfig) -> Option<Self> {
        if config.api_key.is_empty() {
            eprintln!("[WARN] [observability] langsmith provider requires an API key");
            return None;
        }
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build();
        Some(Self {
            api_url: config.api_url.trim_end_matches('/').to_string(),
            api_key: config.api_key.clone(),
            agent,
        })
    }

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

impl TraceBackend for LangSmith {
    fn submit_batch(&self, creates: &[Value], updates: &[Value]) -> FlushResult {
        if creates.is_empty() && updates.is_empty() {
            return FlushResult::Ok;
        }
        let body = json!({ "post": creates, "patch": updates });
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

    fn name(&self) -> &'static str {
        "langsmith"
    }
}
