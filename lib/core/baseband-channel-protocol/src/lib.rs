mod sexp;

pub mod audit;
pub mod channel;
pub mod envelope;
pub mod peer;
pub mod routing;
pub mod security;
pub mod transport;

pub use audit::AuditContext;
pub use channel::ChannelRef;
pub use envelope::{
    capabilities_to_sexp, CanonicalMobileEnvelope, Capability, ChannelBatch, ChannelBody,
    ChannelEnvelope,
};
pub use peer::{ConversationRef, OriginContext, PeerRef, SessionContext};
pub use routing::{ComplexityTier, RoutingContext, UserTier, ROUTING_DIMS};
pub use security::{SecurityContext, SecurityLabel};
pub use transport::TransportContext;

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
        let ctx = RoutingContext {
            tier: ComplexityTier::Simple,
            score: 0.1,
            confidence: 0.9,
            active_tier: UserTier::Eco,
            dimensions: [0.0; 14],
        };
        let copy = ctx;
        assert_eq!(copy.tier, ComplexityTier::Simple);
        assert_eq!(ctx.score, copy.score);
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
