use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit::AuditContext;
use crate::channel::ChannelRef;
use crate::peer::{ConversationRef, OriginContext, PeerRef, SessionContext};
use crate::routing::RoutingContext;
use crate::security::SecurityContext;
use crate::sexp::sexp_string;
use crate::transport::TransportContext;

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
