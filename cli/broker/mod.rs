use async_trait::async_trait;
use console::style;
use harmonia_transport_auth::normalize_fingerprint;
use rmqtt::context::ServerContext;
use rmqtt::hook::{Handler, HookResult, Parameter, ReturnType, Type};
use rmqtt::net::Builder as ListenerBuilder;
use rmqtt::server::MqttServer;
use rmqtt::types::AuthResult;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{OnceLock, RwLock};
use std::thread;
use std::time::Duration;

const BROKER_SCOPE: &str = "mqtt-broker";
const FRONTEND_SCOPE: &str = "mqtt-frontend";
const CONFIG_COMPONENT: &str = "harmonia-cli";
const DEFAULT_REMOTE_CONFIG_URL: &str = "https://harmoniis.com/api/agent";
const DEFAULT_BIND: &str = "127.0.0.1:8883";
const DEFAULT_REMOTE_LABEL: &str = "mqtt-client-alice";
const DEFAULT_REFRESH_SECONDS: u64 = 60;
const REMOTE_CONFIG_QUERY: &str = r#"
query HarmoniaAgentConfig($fingerprint: String!, $publicKey: String!, $signature: String!, $requestedAt: String!) {
  config(
    fingerprint: $fingerprint,
    publicKey: $publicKey,
    signature: $signature,
    requestedAt: $requestedAt
  ) {
    fingerprint
    mqttDomain
    mqttPort
    mqttTlsRequired
    brokerMode
    trustedClientFingerprints
    pushWebhookUrl
    pushWebhookToken
    configJson
    trustedDevices {
      fingerprint
      deviceId
      label
      platform
      pushToken
      snsTargetArn
      pushDataJson
      mqttIdentityFingerprint
      trustedAgentFingerprints
      configJson
      updatedAt
    }
    updatedAt
  }
}
"#;

#[derive(Debug, Default, Clone)]
pub(super) struct BrokerTrustState {
    owner_fingerprint: String,
    trusted_fingerprints: HashSet<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GraphqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct AgentConfigData {
    config: Option<RemoteAgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct RemotePushDevice {
    fingerprint: String,
    #[serde(rename = "deviceId")]
    device_id: String,
    label: Option<String>,
    platform: Option<String>,
    #[serde(rename = "pushToken")]
    push_token: Option<String>,
    #[serde(rename = "snsTargetArn")]
    sns_target_arn: Option<String>,
    #[serde(rename = "pushDataJson")]
    push_data_json: Option<String>,
    #[serde(rename = "mqttIdentityFingerprint")]
    mqtt_identity_fingerprint: Option<String>,
    #[serde(rename = "trustedAgentFingerprints")]
    trusted_agent_fingerprints: Vec<String>,
    #[serde(rename = "configJson")]
    config_json: Option<String>,
    #[serde(rename = "updatedAt")]
    updated_at: String,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct RemoteAgentConfig {
    fingerprint: String,
    #[serde(rename = "mqttDomain")]
    mqtt_domain: String,
    #[serde(rename = "mqttPort")]
    mqtt_port: i32,
    #[serde(rename = "mqttTlsRequired")]
    mqtt_tls_required: bool,
    #[serde(rename = "brokerMode")]
    broker_mode: String,
    #[serde(rename = "trustedClientFingerprints")]
    trusted_client_fingerprints: Vec<String>,
    #[serde(rename = "pushWebhookUrl")]
    push_webhook_url: Option<String>,
    #[serde(rename = "pushWebhookToken")]
    push_webhook_token: Option<String>,
    #[serde(rename = "configJson")]
    config_json: Option<String>,
    #[serde(rename = "trustedDevices")]
    trusted_devices: Vec<RemotePushDevice>,
}

#[derive(Debug)]
pub(super) struct VaultSignResult {
    label: String,
    index: u32,
    public_key: String,
    signature: String,
}

pub(super) fn broker_trust_state() -> &'static RwLock<BrokerTrustState> {
    static STATE: OnceLock<RwLock<BrokerTrustState>> = OnceLock::new();
    STATE.get_or_init(|| RwLock::new(BrokerTrustState::default()))
}

struct TrustedClientAuthHandler;

#[async_trait]
impl Handler for TrustedClientAuthHandler {
    async fn hook(&self, param: &Parameter, _acc: Option<HookResult>) -> ReturnType {
        let Parameter::ClientAuthenticate(connect_info) = param else {
            return (true, None);
        };

        let Some(username) = connect_info.username() else {
            return (
                false,
                Some(HookResult::AuthResult(AuthResult::NotAuthorized)),
            );
        };
        let presented = normalize_fingerprint(username.as_ref());
        let allowed = broker_trust_state()
            .read()
            .map(|state| {
                state.owner_fingerprint == presented
                    || state.trusted_fingerprints.contains(&presented)
            })
            .unwrap_or(false);

        if allowed {
            (
                false,
                Some(HookResult::AuthResult(AuthResult::Allow(false, None))),
            )
        } else {
            (
                false,
                Some(HookResult::AuthResult(AuthResult::NotAuthorized)),
            )
        }
    }
}

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var(
            "HARMONIA_STATE_ROOT",
            crate::paths::data_dir()?.to_string_lossy().to_string(),
        );
    }
    harmonia_config_store::init_v2().map_err(|e| format!("config-store init failed: {e}"))?;

    if let Err(e) = sync_remote_config_once() {
        eprintln!(
            "{} remote MQTT config sync failed before broker start: {}",
            style("!").yellow().bold(),
            e
        );
    }

    let refresh_seconds = config_u64(
        BROKER_SCOPE,
        "remote-config-refresh-seconds",
        DEFAULT_REFRESH_SECONDS,
    );
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(refresh_seconds.max(15)));
        if let Err(e) = sync_remote_config_once() {
            eprintln!(
                "{} remote MQTT config refresh failed: {}",
                style("!").yellow().bold(),
                e
            );
        }
    });

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async move {
        let scx = ServerContext::new().build().await;
        let register = scx.extends.hook_mgr().register();
        register
            .add(Type::ClientAuthenticate, Box::new(TrustedClientAuthHandler))
            .await;
        register.start().await;

        let listener = build_listener()?;
        println!("{} MQTT broker listening", style("→").cyan().bold());
        MqttServer::new(scx)
            .listener(listener)
            .build()
            .run()
            .await?;
        Ok::<(), Box<dyn std::error::Error>>(())
    })
}

pub(super) fn build_listener() -> Result<rmqtt::net::Listener, Box<dyn std::error::Error>> {
    let bind = config_or(BROKER_SCOPE, "bind", DEFAULT_BIND);
    let listen: SocketAddr = bind.parse()?;
    let tls_enabled = config_bool(BROKER_SCOPE, "tls", true);

    let builder = ListenerBuilder::new()
        .name("harmonia-mqtt-broker")
        .laddr(listen)
        .allow_anonymous(false)
        .max_connections(1024)
        .max_handshaking_limit(128)
        .max_packet_size(1024 * 1024)
        .max_mqueue_len(2048)
        .cert_cn_as_username(true);

    if tls_enabled {
        let cert = config_required(BROKER_SCOPE, "server-cert")?;
        let key = config_required(BROKER_SCOPE, "server-key")?;
        Ok(builder
            .tls_cross_certificate(true)
            .tls_cert(Some(cert))
            .tls_key(Some(key))
            .bind()?
            .tls()?)
    } else {
        Ok(builder.bind()?.tcp()?)
    }
}

pub(crate) mod config;
pub(crate) mod runtime;
use config::*;
use runtime::*;

