mod api;
mod bitcoin;
mod challenge;
mod model;
mod policy;
mod settlement;
mod wallet;

pub use api::extract_payment_metadata;
pub use challenge::{
    append_settlement_metadata, build_challenge_payload, build_denied_payload, record_challenge,
    settle_payment,
};
pub use model::{InboundPaymentMetadata, PaymentRequirement, PolicyDecision, SettlementReceipt};
pub use policy::{build_policy_query, default_policy_response, parse_policy_response};

#[cfg(test)]
mod tests {
    use super::{
        build_challenge_payload, extract_payment_metadata, parse_policy_response,
        PaymentRequirement, PolicyDecision,
    };
    use harmonia_baseband_channel_protocol::{
        AuditContext, ChannelBody, ChannelEnvelope, ChannelRef, ConversationRef, PeerRef,
        SecurityContext, SecurityLabel, TransportContext,
    };

    fn sample_envelope(body: &str, metadata: Option<&str>) -> ChannelEnvelope {
        ChannelEnvelope {
            id: 1,
            version: 1,
            kind: "external".to_string(),
            type_name: "message.structured".to_string(),
            conversation: ConversationRef::new("conv"),
            channel: ChannelRef::new("http2", "peer/session/default"),
            peer: PeerRef::new("peer"),
            origin: None,
            session: None,
            body: ChannelBody {
                format: "json".to_string(),
                text: body.to_string(),
                raw: body.to_string(),
            },
            capabilities: vec![],
            security: SecurityContext {
                label: SecurityLabel::Authenticated,
                source: "test".to_string(),
                fingerprint_valid: true,
            },
            audit: AuditContext {
                timestamp_ms: 0,
                dissonance: 0.0,
            },
            attachments: vec![],
            transport: TransportContext {
                kind: "http2".to_string(),
                raw_address: "peer/session/default".to_string(),
                raw_metadata: metadata.map(ToString::to_string),
            },
            routing: None,
        }
    }

    #[test]
    fn extracts_payment_metadata_from_json_and_metadata() {
        let envelope = sample_envelope(
            r#"{"payment":{"proof":"secret-1","action":"post","challenge":"c-1"}}"#,
            Some("(:payment-rail \"voucher\")"),
        );
        let payment = extract_payment_metadata(&envelope);
        assert_eq!(payment.rail.as_deref(), Some("voucher"));
        assert_eq!(payment.proof.as_deref(), Some("secret-1"));
        assert_eq!(payment.action_hint.as_deref(), Some("post"));
        assert_eq!(payment.challenge_id.as_deref(), Some("c-1"));
    }

    #[test]
    fn parses_pay_policy_response() {
        let decision = parse_policy_response(
            "(:mode :pay :action \"post\" :price \"42\" :unit \"credits\" :allowed-rails (\"voucher\" \"bitcoin\") :policy-id \"cfg\")",
        )
        .expect("parse pay response");
        match decision {
            PolicyDecision::Pay(requirement) => {
                assert_eq!(requirement.action, "post");
                assert_eq!(requirement.price, "42");
                assert_eq!(requirement.unit, "credits");
                assert_eq!(requirement.allowed_rails, vec!["voucher", "bitcoin"]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn parses_uppercase_policy_symbols() {
        let decision = parse_policy_response(
            "(:MODE :PAY :ACTION \"post\" :PRICE \"42\" :UNIT \"credits\" :ALLOWED-RAILS (\"voucher\" \"bitcoin\") :POLICY-ID \"cfg\")",
        )
        .expect("parse uppercase pay response");
        match decision {
            PolicyDecision::Pay(requirement) => {
                assert_eq!(requirement.action, "post");
                assert_eq!(requirement.allowed_rails, vec!["voucher", "bitcoin"]);
            }
            other => panic!("unexpected decision: {other:?}"),
        }
    }

    #[test]
    fn challenge_payload_keeps_legacy_fields() {
        let payload = build_challenge_payload(
            &PaymentRequirement {
                action: "post".to_string(),
                price: "42".to_string(),
                unit: "credits".to_string(),
                allowed_rails: vec!["voucher".to_string(), "bitcoin".to_string()],
                challenge_id: Some("challenge-123".to_string()),
                policy_id: Some("cfg".to_string()),
                note: None,
            },
            "payment_required",
            "pay first",
        );
        let parsed: serde_json::Value = serde_json::from_str(&payload).expect("json payload");
        assert_eq!(parsed["challenge_id"], "challenge-123");
        assert_eq!(parsed["payment"]["challenge_id"], "challenge-123");
        assert_eq!(parsed["payment"]["expected_payment_rail"], "voucher");
        assert_eq!(parsed["payment"]["currency"], "voucher");
        assert_eq!(parsed["payment"]["header"], "X-Voucher-Secret");
    }
}
