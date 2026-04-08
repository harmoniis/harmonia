mod api;
mod connection;
mod device;
mod envelope;
mod ffi;
mod frontend;
mod model;
mod publish;
mod queue;

pub use api::{
    harmonia_frontend_free_string, harmonia_frontend_healthcheck, harmonia_frontend_init,
    harmonia_frontend_last_error, harmonia_frontend_poll, harmonia_frontend_send,
    harmonia_frontend_shutdown, harmonia_frontend_version, harmonia_mqtt_client_free_string,
    harmonia_mqtt_client_healthcheck, harmonia_mqtt_client_last_error,
    harmonia_mqtt_client_make_envelope, harmonia_mqtt_client_parse_envelope,
    harmonia_mqtt_client_poll, harmonia_mqtt_client_publish, harmonia_mqtt_client_reset,
    harmonia_mqtt_client_version,
};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::{
        extract_device_id_from_topic, is_device_connect_topic, is_device_disconnect_topic,
    };
    use crate::model::DeviceInfo;
    use crate::model::MessageEnvelope;

    #[test]
    fn healthcheck_returns_one() {
        assert_eq!(harmonia_mqtt_client_healthcheck(), 1);
    }

    #[test]
    fn version_ptr_is_non_null() {
        assert!(!harmonia_mqtt_client_version().is_null());
    }

    #[test]
    fn envelope_roundtrip_v1() {
        let env = MessageEnvelope {
            v: 1,
            kind: "cmd".to_string(),
            type_name: "text_input".to_string(),
            id: "1".to_string(),
            ts: "2026-01-01T00:00:00Z".to_string(),
            agent_fp: "A".to_string(),
            client_fp: "C".to_string(),
            body: serde_json::json!({"text":"hi"}),
        };
        let json = serde_json::to_string(&env).expect("serialize");
        let parsed: MessageEnvelope = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(parsed.v, 1);
        assert_eq!(parsed.kind, "cmd");
    }

    #[test]
    fn device_info_metadata_sexp() {
        let d = DeviceInfo {
            device_id: "dev-1".into(),
            owner_fingerprint: "client-1".into(),
            platform: "ios".into(),
            platform_version: "17.2".into(),
            app_version: "1.0.0".into(),
            device_model: "iPhone 15".into(),
            capabilities: vec!["voice".into(), "location".into()],
            permissions_granted: vec![],
            a2ui_version: "1.0".into(),
            push_token: None,
            mqtt_identity_fingerprint: Some("client-1".into()),
            connected: true,
            last_seen_ms: 0,
        };
        let sexp = d.to_metadata_sexp();
        assert!(sexp.contains(":platform \"ios\""));
        assert!(sexp.contains(":a2ui-version \"1.0\""));
        assert!(sexp.contains("\"voice\" \"location\""));
    }

    #[test]
    fn extract_device_id_works() {
        assert_eq!(
            extract_device_id_from_topic("harmonia/agent1/device/uuid-123/msg"),
            Some("uuid-123".to_string())
        );
        assert_eq!(extract_device_id_from_topic("some/other/topic"), None);
    }

    #[test]
    fn connect_disconnect_topic_detection() {
        assert!(is_device_connect_topic("harmonia/a1/device/d1/connect"));
        assert!(!is_device_connect_topic("harmonia/a1/device/d1/msg"));
        assert!(is_device_disconnect_topic(
            "harmonia/a1/device/d1/disconnect"
        ));
    }
}
