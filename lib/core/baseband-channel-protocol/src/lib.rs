use serde::{Deserialize, Serialize};
use serde_json::Value;

fn escape_sexp_string(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn sexp_string(value: &str) -> String {
    format!("\"{}\"", escape_sexp_string(value))
}

fn sexp_optional_string(key: &str, value: Option<&str>) -> String {
    match value {
        Some(v) if !v.is_empty() => format!(" :{} {}", key, sexp_string(v)),
        _ => String::new(),
    }
}

fn sexp_bool(value: bool) -> &'static str {
    if value {
        "t"
    } else {
        "nil"
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLabel {
    Owner,
    Authenticated,
    Anonymous,
    Untrusted,
}

impl SecurityLabel {
    pub fn from_str(s: &str) -> Self {
        match s {
            "owner" => Self::Owner,
            "authenticated" => Self::Authenticated,
            "anonymous" => Self::Anonymous,
            _ => Self::Untrusted,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Authenticated => "authenticated",
            Self::Anonymous => "anonymous",
            Self::Untrusted => "untrusted",
        }
    }

    pub fn weight(&self) -> f64 {
        match self {
            Self::Owner => 1.0,
            Self::Authenticated => 0.8,
            Self::Anonymous => 0.4,
            Self::Untrusted => 0.1,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalMobileEnvelope {
    pub v: u8,
    pub kind: String,
    #[serde(rename = "type")]
    pub type_name: String,
    pub id: String,
    pub ts: String,
    pub agent_fp: String,
    pub client_fp: String,
    pub body: Value,
}

impl CanonicalMobileEnvelope {
    pub fn body_text(&self) -> String {
        if let Some(text) = self.body.get("text").and_then(|v| v.as_str()) {
            return text.to_string();
        }
        if let Some(payload) = self.body.get("payload").and_then(|v| v.as_str()) {
            return payload.to_string();
        }
        match self.body.as_str() {
            Some(text) => text.to_string(),
            None => self.body.to_string(),
        }
    }

    pub fn body_format(&self) -> &'static str {
        if self.body.get("text").is_some() || self.body.get("payload").is_some() {
            "text"
        } else if self.body.is_string() {
            "text"
        } else {
            "json"
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChannelRef {
    pub kind: String,
    pub address: String,
    pub label: String,
}

impl ChannelRef {
    pub fn new(kind: impl Into<String>, address: impl Into<String>) -> Self {
        let kind = kind.into();
        let address = address.into();
        let label = if address.is_empty() {
            kind.clone()
        } else {
            format!("{}:{}", kind, address)
        };
        Self {
            kind,
            address,
            label,
        }
    }

    pub fn to_sexp(&self) -> String {
        format!(
            "(:kind {} :address {} :label {})",
            sexp_string(&self.kind),
            sexp_string(&self.address),
            sexp_string(&self.label)
        )
    }
}

#[derive(Debug, Clone)]
pub struct PeerRef {
    pub id: String,
    pub origin_fp: Option<String>,
    pub agent_fp: Option<String>,
    pub device_id: Option<String>,
    pub platform: Option<String>,
    pub device_model: Option<String>,
    pub app_version: Option<String>,
    pub a2ui_version: Option<String>,
}

impl PeerRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            origin_fp: None,
            agent_fp: None,
            device_id: None,
            platform: None,
            device_model: None,
            app_version: None,
            a2ui_version: None,
        }
    }

    pub fn to_sexp(&self) -> String {
        format!(
            "(:id {}{}{}{}{}{}{})",
            sexp_string(&self.id),
            sexp_optional_string("origin-fp", self.origin_fp.as_deref()),
            sexp_optional_string("agent-fp", self.agent_fp.as_deref()),
            sexp_optional_string("device-id", self.device_id.as_deref()),
            sexp_optional_string("platform", self.platform.as_deref()),
            sexp_optional_string("device-model", self.device_model.as_deref()),
            sexp_optional_string("app-version", self.app_version.as_deref())
                + &sexp_optional_string("a2ui-version", self.a2ui_version.as_deref())
        )
    }
}

#[derive(Debug, Clone)]
pub struct ConversationRef {
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct OriginContext {
    pub node_id: String,
    pub node_label: Option<String>,
    pub node_role: Option<String>,
    pub channel_class: Option<String>,
    pub node_key_id: Option<String>,
    pub transport_security: Option<String>,
    pub remote: bool,
}

impl OriginContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:node-id {}{}{}{}{}{} :remote {})",
            sexp_string(&self.node_id),
            sexp_optional_string("node-label", self.node_label.as_deref()),
            sexp_optional_string("node-role", self.node_role.as_deref()),
            sexp_optional_string("channel-class", self.channel_class.as_deref()),
            sexp_optional_string("node-key-id", self.node_key_id.as_deref()),
            sexp_optional_string("transport-security", self.transport_security.as_deref()),
            sexp_bool(self.remote)
        )
    }
}

#[derive(Debug, Clone)]
pub struct SessionContext {
    pub id: String,
    pub label: Option<String>,
}

impl SessionContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:id {}{})",
            sexp_string(&self.id),
            sexp_optional_string("label", self.label.as_deref())
        )
    }
}

impl ConversationRef {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn to_sexp(&self) -> String {
        format!("(:id {})", sexp_string(&self.id))
    }
}

#[derive(Debug, Clone)]
pub struct ChannelBody {
    pub format: String,
    pub text: String,
    pub raw: String,
}

impl ChannelBody {
    pub fn text(text: impl Into<String>) -> Self {
        let text = text.into();
        Self {
            format: "text".to_string(),
            raw: text.clone(),
            text,
        }
    }

    pub fn to_sexp(&self) -> String {
        format!(
            "(:format {} :text {} :raw {})",
            sexp_string(&self.format),
            sexp_string(&self.text),
            sexp_string(&self.raw)
        )
    }
}

#[derive(Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub value: String,
}

impl Capability {
    pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            value: value.into(),
        }
    }
}

pub fn capabilities_to_sexp(capabilities: &[Capability]) -> String {
    if capabilities.is_empty() {
        return "nil".to_string();
    }
    let parts: Vec<String> = capabilities
        .iter()
        .map(|cap| format!(":{} {}", cap.name, sexp_string(&cap.value)))
        .collect();
    format!("({})", parts.join(" "))
}

#[derive(Debug, Clone)]
pub struct SecurityContext {
    pub label: SecurityLabel,
    pub source: String,
    pub fingerprint_valid: bool,
}

impl SecurityContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:label {} :source {} :fingerprint-valid {})",
            sexp_string(self.label.as_str()),
            sexp_string(&self.source),
            sexp_bool(self.fingerprint_valid)
        )
    }
}

#[derive(Debug, Clone)]
pub struct AuditContext {
    pub timestamp_ms: u64,
    pub dissonance: f64,
}

impl AuditContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:timestamp-ms {} :dissonance {:.4})",
            self.timestamp_ms, self.dissonance
        )
    }
}

/// Number of scoring dimensions in the complexity encoder.
pub const ROUTING_DIMS: usize = 14;

/// Complexity tier — zero-size discriminant, no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityTier {
    Simple,
    Medium,
    Complex,
    Reasoning,
}

impl ComplexityTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Medium => "medium",
            Self::Complex => "complex",
            Self::Reasoning => "reasoning",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "simple" => Self::Simple,
            "medium" => Self::Medium,
            "complex" => Self::Complex,
            "reasoning" => Self::Reasoning,
            _ => Self::Medium,
        }
    }
}

/// User routing tier — zero-size discriminant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserTier {
    Auto,
    Eco,
    Premium,
    Free,
}

impl UserTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Eco => "eco",
            Self::Premium => "premium",
            Self::Free => "free",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "eco" => Self::Eco,
            "premium" => Self::Premium,
            "free" => Self::Free,
            _ => Self::Auto,
        }
    }
}

/// Routing metadata on every signal — fully stack-allocated (130 bytes).
/// No String, no Vec, no heap indirection.
#[derive(Debug, Clone, Copy)]
pub struct RoutingContext {
    pub tier: ComplexityTier,
    pub score: f64,
    pub confidence: f64,
    pub active_tier: UserTier,
    pub dimensions: [f64; ROUTING_DIMS],
}

impl RoutingContext {
    pub fn to_sexp(&self) -> String {
        use std::fmt::Write;
        let mut out = String::with_capacity(256);
        let _ = write!(
            out,
            "(:tier \"{}\" :score {:.4} :confidence {:.4} :active-tier \"{}\" :dimensions (",
            self.tier.as_str(),
            self.score,
            self.confidence,
            self.active_tier.as_str()
        );
        for (i, d) in self.dimensions.iter().enumerate() {
            if i > 0 {
                out.push(' ');
            }
            let _ = write!(out, "{:.4}", d);
        }
        out.push_str("))");
        out
    }
}

#[derive(Debug, Clone)]
pub struct TransportContext {
    pub kind: String,
    pub raw_address: String,
    pub raw_metadata: Option<String>,
}

impl TransportContext {
    pub fn to_sexp(&self) -> String {
        format!(
            "(:kind {} :raw-address {}{})",
            sexp_string(&self.kind),
            sexp_string(&self.raw_address),
            sexp_optional_string("raw-metadata", self.raw_metadata.as_deref())
        )
    }
}

#[derive(Debug, Clone)]
pub struct ChannelEnvelope {
    pub id: u64,
    pub version: u8,
    pub kind: String,
    pub type_name: String,
    pub channel: ChannelRef,
    pub peer: PeerRef,
    pub conversation: ConversationRef,
    pub origin: Option<OriginContext>,
    pub session: Option<SessionContext>,
    pub body: ChannelBody,
    pub capabilities: Vec<Capability>,
    pub security: SecurityContext,
    pub audit: AuditContext,
    pub attachments: Vec<String>,
    pub transport: TransportContext,
    pub routing: Option<RoutingContext>,
}

impl ChannelEnvelope {
    pub fn to_sexp(&self) -> String {
        let attachments = if self.attachments.is_empty() {
            "nil".to_string()
        } else {
            let items: Vec<String> = self
                .attachments
                .iter()
                .map(|item| sexp_string(item))
                .collect();
            format!("({})", items.join(" "))
        };
        let routing_sexp = self
            .routing
            .as_ref()
            .map(|r| r.to_sexp())
            .unwrap_or_else(|| "nil".to_string());
        format!(
            "(:id {} :version {} :kind {} :type-name {} :channel {} :peer {} :conversation {} :origin {} :session {} :body {} :capabilities {} :security {} :audit {} :attachments {} :transport {} :routing {})",
            self.id,
            self.version,
            sexp_string(&self.kind),
            sexp_string(&self.type_name),
            self.channel.to_sexp(),
            self.peer.to_sexp(),
            self.conversation.to_sexp(),
            self.origin
                .as_ref()
                .map(|origin| origin.to_sexp())
                .unwrap_or_else(|| "nil".to_string()),
            self.session
                .as_ref()
                .map(|session| session.to_sexp())
                .unwrap_or_else(|| "nil".to_string()),
            self.body.to_sexp(),
            capabilities_to_sexp(&self.capabilities),
            self.security.to_sexp(),
            self.audit.to_sexp(),
            attachments,
            self.transport.to_sexp(),
            routing_sexp
        )
    }
}

#[derive(Debug, Clone)]
pub struct ChannelBatch {
    pub envelopes: Vec<ChannelEnvelope>,
    pub poll_timestamp_ms: u64,
}

impl ChannelBatch {
    pub fn to_sexp(&self) -> String {
        if self.envelopes.is_empty() {
            return "nil".to_string();
        }
        let items: Vec<String> = self
            .envelopes
            .iter()
            .map(|envelope| envelope.to_sexp())
            .collect();
        format!("({})", items.join(" "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_context_to_sexp() {
        let ctx = RoutingContext {
            tier: ComplexityTier::Complex,
            score: 0.67,
            confidence: 0.89,
            active_tier: UserTier::Auto,
            dimensions: [
                0.1, 0.8, 0.3, 0.2, 0.0, -0.5, 0.6, 0.2, 0.4, 0.3, 0.1, 0.0, 0.2, 0.5,
            ],
        };
        let sexp = ctx.to_sexp();
        assert!(sexp.contains(":tier \"complex\""), "sexp: {}", sexp);
        assert!(sexp.contains(":score 0.67"), "sexp: {}", sexp);
        assert!(sexp.contains(":confidence 0.89"), "sexp: {}", sexp);
        assert!(sexp.contains(":active-tier \"auto\""), "sexp: {}", sexp);
        assert!(sexp.contains(":dimensions ("), "sexp: {}", sexp);
    }

    #[test]
    fn routing_context_is_stack_allocated() {
        // RoutingContext should be Copy — fully stack-allocated, no heap.
        let ctx = RoutingContext {
            tier: ComplexityTier::Simple,
            score: 0.1,
            confidence: 0.9,
            active_tier: UserTier::Eco,
            dimensions: [0.0; 14],
        };
        let copy = ctx; // Copy, not move
        assert_eq!(copy.tier, ComplexityTier::Simple);
        assert_eq!(ctx.score, copy.score); // both still valid
    }

    #[test]
    fn envelope_with_routing_context() {
        let envelope = ChannelEnvelope {
            id: 42,
            version: 1,
            kind: "external".to_string(),
            type_name: "message.text".to_string(),
            channel: ChannelRef::new("tui", "local"),
            peer: PeerRef::new("user"),
            conversation: ConversationRef::new("conv-1"),
            origin: None,
            session: None,
            body: ChannelBody::text("implement a b-tree"),
            capabilities: Vec::new(),
            security: SecurityContext {
                label: SecurityLabel::Owner,
                source: "test".to_string(),
                fingerprint_valid: true,
            },
            audit: AuditContext {
                timestamp_ms: 1000,
                dissonance: 0.0,
            },
            attachments: Vec::new(),
            transport: TransportContext {
                kind: "tui".to_string(),
                raw_address: "local".to_string(),
                raw_metadata: None,
            },
            routing: Some(RoutingContext {
                tier: ComplexityTier::Complex,
                score: 0.65,
                confidence: 0.82,
                active_tier: UserTier::Auto,
                dimensions: [0.0; 14],
            }),
        };
        let sexp = envelope.to_sexp();
        assert!(
            sexp.contains(":routing (:tier \"complex\""),
            "routing missing from envelope sexp: {}",
            &sexp[..sexp.len().min(200)]
        );
    }

    #[test]
    fn envelope_without_routing_context() {
        let envelope = ChannelEnvelope {
            id: 1,
            version: 1,
            kind: "external".to_string(),
            type_name: "message.text".to_string(),
            channel: ChannelRef::new("tui", "local"),
            peer: PeerRef::new("user"),
            conversation: ConversationRef::new("conv-1"),
            origin: None,
            session: None,
            body: ChannelBody::text("/help"),
            capabilities: Vec::new(),
            security: SecurityContext {
                label: SecurityLabel::Owner,
                source: "test".to_string(),
                fingerprint_valid: true,
            },
            audit: AuditContext {
                timestamp_ms: 1000,
                dissonance: 0.0,
            },
            attachments: Vec::new(),
            transport: TransportContext {
                kind: "tui".to_string(),
                raw_address: "local".to_string(),
                raw_metadata: None,
            },
            routing: None,
        };
        let sexp = envelope.to_sexp();
        assert!(
            sexp.contains(":routing nil"),
            "command envelope should have :routing nil"
        );
    }
}
