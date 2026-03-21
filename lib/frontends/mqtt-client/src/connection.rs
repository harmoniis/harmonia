use harmonia_transport_auth::{
    load_client_auth_from_config_or_vault, load_required_bytes, record_tls_lineage_seed,
};
use rumqttc::{Client, MqttOptions, TlsConfiguration, Transport};
use std::process;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::model::COMPONENT;

pub(crate) fn parse_broker() -> Result<(String, u16), String> {
    let raw = harmonia_config_store::get_own_or(COMPONENT, "broker", "test.mosquitto.org:1883")
        .unwrap_or_else(|_| "test.mosquitto.org:1883".to_string());
    let (host, port_raw) = raw
        .split_once(':')
        .ok_or_else(|| format!("invalid HARMONIA_MQTT_BROKER: {raw}"))?;
    let port = port_raw
        .parse::<u16>()
        .map_err(|e| format!("invalid mqtt port: {e}"))?;
    Ok((host.to_string(), port))
}

pub(crate) fn timeout_ms() -> u64 {
    harmonia_config_store::get_own(COMPONENT, "timeout-ms")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(5000)
}

pub(crate) fn tls_enabled() -> bool {
    harmonia_config_store::get_own(COMPONENT, "tls")
        .ok()
        .flatten()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

pub(crate) fn client_id(prefix: &str) -> String {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    format!("harmonia-{prefix}-{}-{ts}", process::id())
}

pub(crate) fn connect(prefix: &str) -> Result<(Client, rumqttc::Connection), String> {
    let (host, port) = parse_broker()?;
    let mut opts = MqttOptions::new(client_id(prefix), host, port);
    opts.set_keep_alive(Duration::from_secs(5));
    if tls_enabled() {
        let ca = load_required_bytes(COMPONENT, "ca-cert")?;
        let _ = record_tls_lineage_seed(COMPONENT, "tls", "mqtt_tls_master_seed");
        let client_auth = load_client_auth_from_config_or_vault(
            COMPONENT,
            "client-cert",
            "client-key",
            "mqtt_tls_client_cert_pem",
            "mqtt_tls_client_key_pem",
        )?;
        opts.set_transport(Transport::Tls(TlsConfiguration::Simple {
            ca,
            alpn: None,
            client_auth,
        }));
    }
    Ok(Client::new(opts, 10))
}
