use async_trait::async_trait;
use console::style;
use harmonia_transport_auth::normalize_fingerprint;
use rmqtt::context::ServerContext;
use rmqtt::hook::{Handler, HookResult, Parameter, ReturnType, Type};
use rmqtt::net::Builder as ListenerBuilder;
use rmqtt::server::MqttServer;
use rmqtt::types::AuthResult;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{OnceLock, RwLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
struct BrokerTrustState {
    owner_fingerprint: String,
    trusted_fingerprints: HashSet<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlResponse<T> {
    data: Option<T>,
    errors: Option<Vec<GraphqlError>>,
}

#[derive(Debug, Deserialize)]
struct GraphqlError {
    message: String,
}

#[derive(Debug, Deserialize)]
struct AgentConfigData {
    config: Option<RemoteAgentConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RemotePushDevice {
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
struct RemoteAgentConfig {
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
struct VaultSignResult {
    label: String,
    index: u32,
    public_key: String,
    signature: String,
}

fn broker_trust_state() -> &'static RwLock<BrokerTrustState> {
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

fn build_listener() -> Result<rmqtt::net::Listener, Box<dyn std::error::Error>> {
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

fn sync_remote_config_once() -> Result<(), Box<dyn std::error::Error>> {
    let remote_config_url = config_or(BROKER_SCOPE, "remote-config-url", DEFAULT_REMOTE_CONFIG_URL);
    if remote_config_url.trim().is_empty() {
        refresh_trust_state_from_local_config();
        return Ok(());
    }
    let wallet = resolve_wallet_db_path();
    let identity_label = config_or(
        BROKER_SCOPE,
        "remote-config-identity-label",
        DEFAULT_REMOTE_LABEL,
    );

    let bootstrap = sign_with_vault(&wallet, &identity_label, "harmonia:init")?;
    let owner_fingerprint = normalize_fingerprint(&bootstrap.public_key);
    let requested_at = now_unix_ms().to_string();
    let message = format!(
        "harmonia:agent-config:get:{}:{}",
        owner_fingerprint, requested_at
    );
    let signed = sign_with_vault(&wallet, &identity_label, &message)?;

    let req_body = json!({
        "query": REMOTE_CONFIG_QUERY,
        "variables": {
            "fingerprint": owner_fingerprint,
            "publicKey": signed.public_key,
            "signature": signed.signature,
            "requestedAt": requested_at,
        }
    });

    let response: GraphqlResponse<AgentConfigData> = ureq::post(&remote_config_url)
        .set("Content-Type", "application/json")
        .send_json(req_body)
        .map_err(|e| format!("remote config request failed: {e}"))?
        .into_json()
        .map_err(|e| format!("remote config decode failed: {e}"))?;

    if let Some(errors) = response.errors {
        let message = errors
            .into_iter()
            .map(|e| e.message)
            .collect::<Vec<_>>()
            .join("; ");
        return Err(format!("remote config API error: {message}").into());
    }

    let Some(data) = response.data else {
        refresh_trust_state_from_local_config();
        return Ok(());
    };
    let Some(config) = data.config else {
        refresh_trust_state_from_local_config();
        return Ok(());
    };

    let broker = format!("{}:{}", config.mqtt_domain, config.mqtt_port.max(1));
    let mut trusted_fps: Vec<String> = config
        .trusted_devices
        .iter()
        .map(|device| {
            device
                .mqtt_identity_fingerprint
                .clone()
                .filter(|fp| !fp.trim().is_empty())
                .unwrap_or_else(|| device.fingerprint.clone())
        })
        .collect();
    if trusted_fps.is_empty() {
        trusted_fps = config.trusted_client_fingerprints.clone();
    }
    trusted_fps = trusted_fps
        .into_iter()
        .map(|fp| normalize_fingerprint(&fp))
        .collect();
    trusted_fps.sort();
    trusted_fps.dedup();

    set_config(FRONTEND_SCOPE, "broker", &broker)?;
    set_config(
        FRONTEND_SCOPE,
        "tls",
        if config.mqtt_tls_required { "1" } else { "0" },
    )?;
    set_config(BROKER_SCOPE, "mode", &config.broker_mode)?;
    set_config(
        FRONTEND_SCOPE,
        "trusted-client-fingerprints-json",
        &serde_json::to_string(&trusted_fps)?,
    )?;
    set_config(
        FRONTEND_SCOPE,
        "trusted-device-registry-json",
        &serde_json::to_string(&config.trusted_devices)?,
    )?;
    if let Some(url) = config.push_webhook_url {
        set_config(FRONTEND_SCOPE, "push-webhook-url", &url)?;
        set_config(BROKER_SCOPE, "push-webhook-url", &url)?;
    }
    if let Some(token) = config.push_webhook_token {
        set_config(FRONTEND_SCOPE, "push-webhook-token", &token)?;
        set_config(BROKER_SCOPE, "push-webhook-token", &token)?;
    }
    if let Some(config_json) = config.config_json {
        set_config(BROKER_SCOPE, "remote-config-json", &config_json)?;
    }
    set_config(BROKER_SCOPE, "last-sync-fingerprint", &config.fingerprint)?;
    set_config(BROKER_SCOPE, "last-sync-at-ms", &now_unix_ms().to_string())?;
    set_config(BROKER_SCOPE, "last-sync-identity-label", &signed.label)?;
    set_config(
        BROKER_SCOPE,
        "last-sync-identity-index",
        &signed.index.to_string(),
    )?;

    let owner_fingerprint = normalize_fingerprint(&signed.public_key);
    update_trust_state(owner_fingerprint, trusted_fps);
    Ok(())
}

fn refresh_trust_state_from_local_config() {
    let owner = config_or(BROKER_SCOPE, "identity-public-key", "");
    let trusted = harmonia_config_store::get_config(
        CONFIG_COMPONENT,
        FRONTEND_SCOPE,
        "trusted-client-fingerprints-json",
    )
    .ok()
    .flatten()
    .unwrap_or_else(|| "[]".to_string());
    let trusted = serde_json::from_str::<Vec<String>>(&trusted)
        .unwrap_or_default()
        .into_iter()
        .map(|fp| normalize_fingerprint(&fp))
        .collect::<Vec<_>>();
    update_trust_state(normalize_fingerprint(&owner), trusted);
}

fn update_trust_state(owner_fingerprint: String, trusted_fingerprints: Vec<String>) {
    if let Ok(mut state) = broker_trust_state().write() {
        state.owner_fingerprint = owner_fingerprint;
        state.trusted_fingerprints = trusted_fingerprints.into_iter().collect();
    }
}

fn sign_with_vault(
    wallet: &PathBuf,
    label: &str,
    message: &str,
) -> Result<VaultSignResult, Box<dyn std::error::Error>> {
    let output = Command::new(resolve_hrmw_bin())
        .args([
            "key",
            "vault-sign",
            "--wallet",
            wallet.to_string_lossy().as_ref(),
            "--label",
            label,
            "--message",
            message,
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "hrmw key vault-sign failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    parse_vault_sign_output(&String::from_utf8_lossy(&output.stdout))
}

fn resolve_hrmw_bin() -> String {
    std::env::var("HARMONIA_HRMW_BIN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            std::env::var("HRMW_BIN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .unwrap_or_else(|| "hrmw".to_string())
}

fn parse_vault_sign_output(output: &str) -> Result<VaultSignResult, Box<dyn std::error::Error>> {
    let label = parse_output_field(output, "Vault label:")?;
    let index = parse_output_field(output, "Vault index:")?.parse::<u32>()?;
    let public_key = parse_output_field(output, "Vault public key:")?;
    let signature = parse_output_field(output, "Signature:")?;
    Ok(VaultSignResult {
        label,
        index,
        public_key,
        signature,
    })
}

fn parse_output_field(output: &str, prefix: &str) -> Result<String, Box<dyn std::error::Error>> {
    for line in output.lines() {
        if let Some(rest) = line.strip_prefix(prefix) {
            return Ok(rest.trim().to_string());
        }
    }
    Err(format!("missing field in hrmw output: {prefix}").into())
}

fn resolve_wallet_db_path() -> PathBuf {
    crate::paths::wallet_db_path().unwrap_or_else(|_| {
        std::env::var("HARMONIA_VAULT_WALLET_DB")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("master.db"))
    })
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn config_or(scope: &str, key: &str, default: &str) -> String {
    harmonia_config_store::get_config_or(CONFIG_COMPONENT, scope, key, default)
        .unwrap_or_else(|_| default.to_string())
}

fn config_required(scope: &str, key: &str) -> Result<String, Box<dyn std::error::Error>> {
    let value = harmonia_config_store::get_config(CONFIG_COMPONENT, scope, key)
        .map_err(|e| format!("config-store read failed for {scope}/{key}: {e}"))?
        .unwrap_or_default();
    if value.trim().is_empty() {
        return Err(format!("missing required config {scope}/{key}").into());
    }
    Ok(value)
}

fn config_bool(scope: &str, key: &str, default: bool) -> bool {
    let value = config_or(scope, key, if default { "1" } else { "0" });
    value.trim().eq_ignore_ascii_case("1") || value.trim().eq_ignore_ascii_case("true")
}

fn config_u64(scope: &str, key: &str, default: u64) -> u64 {
    config_or(scope, key, &default.to_string())
        .parse::<u64>()
        .unwrap_or(default)
}

fn set_config(scope: &str, key: &str, value: &str) -> Result<(), Box<dyn std::error::Error>> {
    harmonia_config_store::set_config(CONFIG_COMPONENT, scope, key, value)
        .map_err(|e| format!("config-store write failed for {scope}/{key}: {e}").into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_vault_sign_output_works() {
        let out = "Vault label:     mqtt-client-alice\nVault index:     1\nVault public key: ABCDEF\nSignature:       1234\n";
        let parsed = parse_vault_sign_output(out).expect("parse");
        assert_eq!(parsed.label, "mqtt-client-alice");
        assert_eq!(parsed.index, 1);
        assert_eq!(parsed.public_key, "ABCDEF");
        assert_eq!(parsed.signature, "1234");
    }

    #[test]
    fn normalize_fingerprint_uppercases_and_strips_separators() {
        assert_eq!(normalize_fingerprint("ab:cd-ef"), "ABCDEF");
    }
}
