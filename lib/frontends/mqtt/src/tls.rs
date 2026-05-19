use std::path::PathBuf;
use std::sync::Arc;

pub(crate) const COMPONENT: &str = "mqtt-frontend";
pub(crate) const LINEAGE_SYMBOL: &str = "mqtt_tls_master_seed";

#[derive(Clone, Debug)]
pub(crate) struct MqttFrontendConfig {
    pub(crate) broker_host: String,
    pub(crate) broker_port: u16,
    pub(crate) agent_fp: String,
    pub(crate) tls: Option<MqttTlsConfig>,
    pub(crate) subscribe_topics: Vec<String>,
    pub(crate) keep_alive_secs: u64,
}

#[derive(Clone, Debug)]
pub(crate) struct MqttTlsConfig {
    pub(crate) ca_cert: PathBuf,
    pub(crate) client_cert: PathBuf,
    pub(crate) client_key: PathBuf,
}

pub(crate) fn load_config() -> Result<MqttFrontendConfig, String> {
    harmonia_config_store::init().map_err(|e| format!("config-store init failed: {e}"))?;
    let _ = harmonia_transport_auth::record_tls_lineage_seed(COMPONENT, "tls", LINEAGE_SYMBOL);

    let broker_url = harmonia_config_store::get_own(COMPONENT, "broker")
        .ok()
        .flatten()
        .ok_or_else(|| "mqtt-frontend/broker not configured".to_string())?;
    let (broker_host, broker_port) = parse_broker_url(&broker_url)?;

    let agent_fp = harmonia_vault::init_from_env()
        .ok()
        .and_then(|_| {
            harmonia_vault::get_secret_for_component(COMPONENT, "mqtt-agent-fp")
                .ok()
                .flatten()
        })
        .or_else(|| {
            harmonia_config_store::get_own(COMPONENT, "agent-fp")
                .ok()
                .flatten()
        })
        .ok_or_else(|| "mqtt-frontend agent fingerprint not configured".to_string())?;

    let tls_required = harmonia_config_store::get_own(COMPONENT, "tls")
        .ok()
        .flatten()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let tls = if tls_required {
        Some(MqttTlsConfig {
            ca_cert: harmonia_transport_auth::required_config_path(COMPONENT, "ca-cert")?,
            client_cert: harmonia_transport_auth::required_config_path(COMPONENT, "client-cert")?,
            client_key: harmonia_transport_auth::required_config_path(COMPONENT, "client-key")?,
        })
    } else {
        None
    };

    let subscribe_topics = vec![
        format!("harmonia/{agent_fp}/inbox/+"),
        format!("harmonia/{agent_fp}/system"),
    ];
    let keep_alive_secs = harmonia_config_store::get_own(COMPONENT, "keep-alive-secs")
        .ok()
        .flatten()
        .and_then(|v| v.parse::<u64>().ok())
        .unwrap_or(30);

    Ok(MqttFrontendConfig {
        broker_host,
        broker_port,
        agent_fp,
        tls,
        subscribe_topics,
        keep_alive_secs,
    })
}

fn parse_broker_url(url: &str) -> Result<(String, u16), String> {
    let trimmed = url.trim();
    let without_scheme = trimmed
        .strip_prefix("mqtts://")
        .or_else(|| trimmed.strip_prefix("mqtt://"))
        .unwrap_or(trimmed);
    let (host, port) = match without_scheme.rsplit_once(':') {
        Some((h, p)) => (
            h.to_string(),
            p.parse::<u16>()
                .map_err(|e| format!("invalid mqtt port in broker URL: {e}"))?,
        ),
        None => (without_scheme.to_string(), 8883_u16),
    };
    if host.is_empty() {
        return Err("mqtt broker URL host is empty".to_string());
    }
    Ok((host, port))
}

pub(crate) fn build_rustls_client_config(tls: &MqttTlsConfig) -> Result<Arc<rustls::ClientConfig>, String> {
    let roots = harmonia_transport_auth::load_root_store(&tls.ca_cert)?;
    let client_cert_chain = harmonia_transport_auth::load_cert_chain(&tls.client_cert)?;
    let client_key = harmonia_transport_auth::load_private_key(&tls.client_key)?;

    let config = rustls::ClientConfig::builder()
        .with_root_certificates(roots)
        .with_client_auth_cert(client_cert_chain, client_key)
        .map_err(|e| format!("client TLS config failed: {e}"))?;
    Ok(Arc::new(config))
}
