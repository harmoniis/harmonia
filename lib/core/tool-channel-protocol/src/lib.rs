//! Harmonia Tool Channel Protocol
//!
//! Standardised types for tool channel invocation within the Harmonia signal
//! architecture. Tools are request/response channels (unlike frontends which
//! are event-driven poll/send channels). Each tool exposes operations via a
//! standard C-ABI contract, and results flow through the baseband as typed
//! ChannelEnvelope signals with dissonance scoring.

use harmonia_baseband_channel_protocol::{
    AuditContext, ChannelBody, ChannelEnvelope, ChannelRef, ConversationRef, PeerRef,
    SecurityContext, SecurityLabel, TransportContext,
};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ToolRequest {
    pub id: u64,
    pub tool_name: String,
    pub operation: String,
    pub params_sexp: String,
    pub timestamp_ms: u64,
}

#[derive(Debug, Clone)]
pub enum ToolStatus {
    Success,
    Error(String),
    Timeout,
}

impl ToolStatus {
    pub fn is_success(&self) -> bool {
        matches!(self, ToolStatus::Success)
    }

    pub fn to_sexp(&self) -> String {
        match self {
            ToolStatus::Success => ":success".to_string(),
            ToolStatus::Error(msg) => format!("(:error \"{}\")", escape_sexp(msg)),
            ToolStatus::Timeout => ":timeout".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ToolResult {
    pub request_id: u64,
    pub tool_name: String,
    pub operation: String,
    pub status: ToolStatus,
    pub body: ChannelBody,
    pub duration_ms: u64,
    pub dissonance: f64,
}

impl ToolResult {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:request-id {} :tool \"{}\" :operation \"{}\" :status {} :body {} :duration-ms {} :dissonance {:.4})",
            self.request_id,
            escape_sexp(&self.tool_name),
            escape_sexp(&self.operation),
            self.status.to_sexp(),
            self.body.to_sexp(),
            self.duration_ms,
            self.dissonance
        )
    }
}

#[derive(Debug, Clone)]
pub struct ToolCapability {
    pub operation: String,
    pub description: String,
    pub params: Vec<ToolParam>,
    pub returns: String,
}

impl ToolCapability {
    pub fn to_sexp(&self) -> String {
        let params_sexp: Vec<String> = self.params.iter().map(|p| p.to_sexp()).collect();
        format!(
            "(:operation \"{}\" :description \"{}\" :params ({}) :returns \"{}\")",
            escape_sexp(&self.operation),
            escape_sexp(&self.description),
            params_sexp.join(" "),
            escape_sexp(&self.returns)
        )
    }
}

#[derive(Debug, Clone)]
pub struct ToolParam {
    pub name: String,
    pub kind: String,
    pub required: bool,
}

impl ToolParam {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:name \"{}\" :kind \"{}\" :required {})",
            escape_sexp(&self.name),
            escape_sexp(&self.kind),
            if self.required { "t" } else { "nil" }
        )
    }
}

// ── Envelope Construction ──────────────────────────────────────────────────

static TOOL_REQUEST_COUNTER: AtomicU64 = AtomicU64::new(1);

pub fn next_request_id() -> u64 {
    TOOL_REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn build_tool_envelope(result: &ToolResult) -> ChannelEnvelope {
    let type_name = format!("tool.{}.{}", result.tool_name, result.operation);
    ChannelEnvelope {
        id: result.request_id,
        version: 1,
        kind: "tool-result".to_string(),
        type_name,
        channel: ChannelRef::new("tool", &result.tool_name),
        peer: PeerRef::new(format!("tool:{}", result.tool_name)),
        conversation: ConversationRef::new(format!(
            "tool:{}:{}",
            result.tool_name, result.request_id
        )),
        origin: None,
        session: None,
        body: result.body.clone(),
        capabilities: Vec::new(),
        security: SecurityContext {
            label: SecurityLabel::Authenticated,
            source: "tool-gateway".to_string(),
            fingerprint_valid: true,
        },
        audit: AuditContext {
            timestamp_ms: now_ms(),
            dissonance: result.dissonance,
        },
        attachments: Vec::new(),
        transport: TransportContext {
            kind: "tool".to_string(),
            raw_address: result.tool_name.clone(),
            raw_metadata: Some(format!(
                "(:operation \"{}\" :duration-ms {} :status {})",
                escape_sexp(&result.operation),
                result.duration_ms,
                result.status.to_sexp()
            )),
        },
        routing: None,
    }
}

pub fn wrap_tool_output(raw_output: &str, tool_name: &str) -> (String, f64) {
    let report = harmonia_signal_integrity::scan_for_injection(raw_output);
    let dissonance = harmonia_signal_integrity::compute_dissonance(&report);
    let wrapped =
        harmonia_signal_integrity::wrap_secure(raw_output, &format!("tool:{}", tool_name));
    (wrapped, dissonance)
}

// ── Utilities ──────────────────────────────────────────────────────────────

fn escape_sexp(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

pub fn capabilities_to_sexp(caps: &[ToolCapability]) -> String {
    if caps.is_empty() {
        return "nil".to_string();
    }
    let items: Vec<String> = caps.iter().map(|c| c.to_sexp()).collect();
    format!("({})", items.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_status_success_sexp() {
        assert_eq!(ToolStatus::Success.to_sexp(), ":success");
    }

    #[test]
    fn tool_status_error_sexp() {
        assert_eq!(
            ToolStatus::Error("fail".into()).to_sexp(),
            "(:error \"fail\")"
        );
    }

    #[test]
    fn tool_param_sexp() {
        let p = ToolParam {
            name: "q".into(),
            kind: "string".into(),
            required: true,
        };
        assert_eq!(p.to_sexp(), "(:name \"q\" :kind \"string\" :required t)");
    }

    #[test]
    fn request_id_increments() {
        let a = next_request_id();
        let b = next_request_id();
        assert!(b > a);
    }
}
