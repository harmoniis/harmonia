//! OTLP/HTTP trace provider.
//!
//! Sends traces as OpenTelemetry Protocol JSON over HTTP.
//! Works with OpenObserve, Jaeger, Grafana Tempo, or any OTLP collector.
//!
//! Config:
//!   backend = "otlp"  (or "openobserve")
//!   api-url = "http://localhost:5080/api/default"
//!   api-key = "user:password"   (Basic auth) or bearer token or empty
//!   project-name = "harmonia"   (maps to service.name)

use std::collections::HashMap;

use crate::backend::{FlushResult, TraceBackend};
use crate::config::ObservabilityConfig;
use serde_json::{json, Value};

pub struct Otlp {
    endpoint: String,
    auth_header: String,
    service_name: String,
    agent: ureq::Agent,
}

impl Otlp {
    pub fn from_config(config: &ObservabilityConfig) -> Option<Self> {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(10))
            .build();

        let base = config.api_url.trim_end_matches('/');
        let endpoint = format!("{}/v1/traces", base);

        let auth_header = if config.api_key.contains(':') {
            format!("Basic {}", base64_encode(config.api_key.as_bytes()))
        } else if !config.api_key.is_empty() {
            format!("Bearer {}", config.api_key)
        } else {
            String::new()
        };

        Some(Self {
            endpoint,
            auth_header,
            service_name: config.project_name.clone(),
            agent,
        })
    }

    fn to_otlp_request(&self, creates: &[Value], updates: &[Value]) -> Value {
        let mut update_map: HashMap<&str, &Value> = HashMap::new();
        for u in updates {
            if let Some(id) = u["id"].as_str() {
                update_map.insert(id, u);
            }
        }

        let mut otlp_spans = Vec::new();

        for create in creates {
            let id = create["id"].as_str().unwrap_or("");
            let trace_id_raw = create["trace_id"].as_str().unwrap_or(id);
            let parent_raw = create["parent_run_id"].as_str();

            let update = update_map.remove(id);

            let end_time = update
                .and_then(|u| u["end_time"].as_str())
                .or_else(|| create["end_time"].as_str());
            let status = update
                .and_then(|u| u["status"].as_str())
                .or_else(|| create["status"].as_str())
                .unwrap_or("success");
            let outputs = update
                .and_then(|u| u.get("outputs"))
                .or_else(|| create.get("outputs"));

            let mut attributes = Vec::new();
            attr_str(&mut attributes, "run_type", create["run_type"].as_str().unwrap_or("chain"));
            attr_str(&mut attributes, "session_name", create["session_name"].as_str().unwrap_or(""));
            if let Some(inputs) = create.get("inputs") {
                attr_str(&mut attributes, "inputs", &inputs.to_string());
            }
            if let Some(o) = outputs {
                attr_str(&mut attributes, "outputs", &o.to_string());
            }

            otlp_spans.push(json!({
                "traceId": uuid_to_trace_id(trace_id_raw),
                "spanId": uuid_to_span_id(id),
                "parentSpanId": parent_raw.map(uuid_to_span_id).unwrap_or_default(),
                "name": create["name"].as_str().unwrap_or(""),
                "kind": 1,
                "startTimeUnixNano": iso_to_nanos(create["start_time"].as_str().unwrap_or("")),
                "endTimeUnixNano": iso_to_nanos(end_time.unwrap_or("")),
                "attributes": attributes,
                "status": { "code": if status == "error" { 2 } else { 1 } },
            }));
        }

        // Orphan updates (span ended but create was in a previous batch)
        for (id, update) in &update_map {
            let trace_id_raw = update["trace_id"].as_str().unwrap_or(id);
            let status = update["status"].as_str().unwrap_or("success");
            otlp_spans.push(json!({
                "traceId": uuid_to_trace_id(trace_id_raw),
                "spanId": uuid_to_span_id(id),
                "name": format!("span-end-{}", &id[..8.min(id.len())]),
                "kind": 1,
                "startTimeUnixNano": iso_to_nanos(update["end_time"].as_str().unwrap_or("")),
                "endTimeUnixNano": iso_to_nanos(update["end_time"].as_str().unwrap_or("")),
                "attributes": [],
                "status": { "code": if status == "error" { 2 } else { 1 } },
            }));
        }

        json!({
            "resourceSpans": [{
                "resource": {
                    "attributes": [
                        { "key": "service.name", "value": { "stringValue": &self.service_name } }
                    ]
                },
                "scopeSpans": [{
                    "scope": { "name": "harmonia-observability" },
                    "spans": otlp_spans
                }]
            }]
        })
    }
}

impl TraceBackend for Otlp {
    fn submit_batch(&self, creates: &[Value], updates: &[Value]) -> FlushResult {
        if creates.is_empty() && updates.is_empty() {
            return FlushResult::Ok;
        }

        let body = self.to_otlp_request(creates, updates);
        let mut req = self
            .agent
            .post(&self.endpoint)
            .set("Content-Type", "application/json");
        if !self.auth_header.is_empty() {
            req = req.set("Authorization", &self.auth_header);
        }

        match req.send_json(body) {
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
        "otlp"
    }
}

// ── Helpers ──────────────────────────────────────────────────────────

fn uuid_to_trace_id(uuid: &str) -> String {
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() >= 32 { hex[..32].to_string() } else { format!("{:0>32}", hex) }
}

fn uuid_to_span_id(uuid: &str) -> String {
    let hex: String = uuid.chars().filter(|c| c.is_ascii_hexdigit()).collect();
    if hex.len() >= 16 { hex[..16].to_string() } else { format!("{:0>16}", hex) }
}

fn iso_to_nanos(iso: &str) -> String {
    if iso.is_empty() { return "0".to_string(); }
    let parts: Vec<&str> = iso.split('T').collect();
    if parts.len() != 2 { return "0".to_string(); }
    let d: Vec<u64> = parts[0].split('-').filter_map(|s| s.parse().ok()).collect();
    if d.len() != 3 { return "0".to_string(); }
    let time_str = parts[1].trim_end_matches('Z');
    let tm: Vec<&str> = time_str.split('.').collect();
    let hms: Vec<u64> = tm[0].split(':').filter_map(|s| s.parse().ok()).collect();
    if hms.len() != 3 { return "0".to_string(); }
    let millis: u64 = if tm.len() > 1 { tm[1].parse().unwrap_or(0) } else { 0 };
    let mut days = 0u64;
    for y in 1970..d[0] { days += if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 { 366 } else { 365 }; }
    let md: [u64; 12] = if (d[0] % 4 == 0 && d[0] % 100 != 0) || d[0] % 400 == 0 {
        [31,29,31,30,31,30,31,31,30,31,30,31]
    } else { [31,28,31,30,31,30,31,31,30,31,30,31] };
    for i in 0..(d[1].saturating_sub(1) as usize).min(12) { days += md[i]; }
    days += d[2].saturating_sub(1);
    let secs = days * 86400 + hms[0] * 3600 + hms[1] * 60 + hms[2];
    (secs * 1_000_000_000 + millis * 1_000_000).to_string()
}

fn attr_str(attrs: &mut Vec<Value>, key: &str, val: &str) {
    attrs.push(json!({ "key": key, "value": { "stringValue": val } }));
}

fn base64_encode(input: &[u8]) -> String {
    const C: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((input.len() + 2) / 3 * 4);
    for chunk in input.chunks(3) {
        let (b0, b1, b2) = (chunk[0] as u32, chunk.get(1).copied().unwrap_or(0) as u32, chunk.get(2).copied().unwrap_or(0) as u32);
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(C[((n >> 18) & 63) as usize] as char);
        out.push(C[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { C[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { C[(n & 63) as usize] as char } else { '=' });
    }
    out
}
