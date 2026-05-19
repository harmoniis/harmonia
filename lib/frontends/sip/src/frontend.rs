//! [`SipFrontend`] — kamailio-bound SIP UAC over plain TCP.
//!
//! REGISTER on init, MESSAGE method in/out. Authentication is standard
//! SIP digest auth (RFC 3261 §22): the agent reads its provisioner-
//! generated password out of vault under `sip-frontend/sip-password`,
//! kamailio's `auth_db` issues a 401 challenge, and the agent answers
//! with the digest response. Username = the agent's PGP fingerprint
//! (uppercase hex), matching the `subscriber.username` row the
//! provisioner inserted at agent-create time.

use async_trait::async_trait;
use md5::{Digest, Md5};

use harmonia_frontend_trait::{Frontend, InboundMessage, PollResult};

use crate::transport::{
    connect, parse_challenge, parse_inbound_messages, SipConnection, SipTcpConfig,
};

const COMPONENT: &str = "sip-frontend";

pub struct SipFrontend {
    conn: SipConnection,
    agent_uri: String,
    realm_default: String,
    password: String,
    cseq: u32,
    call_id_seed: String,
}

fn config_required(key: &str) -> Result<String, String> {
    harmonia_config_store::get_own(COMPONENT, key)
        .ok()
        .flatten()
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| format!("sip-frontend/{key} not configured"))
}

fn read_password() -> Result<String, String> {
    harmonia_vault::init_from_env()?;
    harmonia_vault::get_secret_for_component(COMPONENT, "sip-password")
        .map_err(|e| format!("vault: {e}"))?
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| {
            "sip-frontend/sip-password missing from vault — provisioner should have written it"
                .to_string()
        })
}

#[async_trait]
impl Frontend for SipFrontend {
    type Config = ();

    fn name() -> &'static str {
        "sip"
    }

    async fn init(_: ()) -> Result<Self, String> {
        // Cluster-internal Consul DNS — `kamailio.service.consul:5060`
        // by default. Override `kamailio-host` in config-store for
        // testing against a local SIP server.
        let host = config_required("kamailio-host")
            .unwrap_or_else(|_| "kamailio.service.consul".to_string());
        let port: u16 = harmonia_config_store::get_own(COMPONENT, "kamailio-port")
            .ok()
            .flatten()
            .and_then(|v| v.parse().ok())
            .unwrap_or(5060);
        let realm = config_required("realm")?;
        let agent_uri = config_required("agent-sip-uri")?;
        let password = read_password()?;

        let conn = connect(&SipTcpConfig {
            host: host.clone(),
            port,
        })
        .await?;

        let mut frontend = Self {
            conn,
            agent_uri,
            realm_default: realm.clone(),
            password,
            cseq: 0,
            call_id_seed: format!(
                "{:x}{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0)
            ),
        };
        frontend.register(3600).await?;
        eprintln!(
            "[INFO] [sip] frontend ready (kamailio={host}:{port} realm={realm} agent={})",
            frontend.agent_uri
        );
        Ok(frontend)
    }

    async fn poll(&mut self) -> PollResult {
        let raw = self.conn.read_available().await?;
        let Some(raw) = raw else { return Ok(Vec::new()) };
        let inbound = parse_inbound_messages(&raw);
        Ok(inbound
            .into_iter()
            .map(|m| InboundMessage {
                metadata: Some(format!(
                    "(:channel-class \"sip\" :node-id \"{}\" :remote t :transport-security \"plain\")",
                    m.from.replace('\\', "\\\\").replace('"', "\\\"")
                )),
                address: m.from,
                text: m.body,
            })
            .collect())
    }

    async fn send(&mut self, channel: &str, text: &str) -> Result<(), String> {
        // `channel` is a destination SIP URI like `sip:user@example.com`.
        // No PGP signing at this hop — kamailio is reachable only from
        // sibling pots over the cluster LAN, and the digest auth on
        // REGISTER established our identity.
        self.cseq = self.cseq.wrapping_add(1);
        let call_id = format!("{}-{}", self.call_id_seed, self.cseq);
        let request = format!(
            "MESSAGE {channel} SIP/2.0\r\n\
             Via: SIP/2.0/TCP {realm};branch=z9hG4bK-{cseq}\r\n\
             Max-Forwards: 70\r\n\
             From: <{agent}>;tag={cseq}\r\n\
             To: <{channel}>\r\n\
             Call-ID: {call_id}\r\n\
             CSeq: {cseq} MESSAGE\r\n\
             Content-Type: text/plain\r\n\
             Content-Length: {body_len}\r\n\
             \r\n\
             {text}",
            realm = self.realm_default,
            agent = self.agent_uri,
            cseq = self.cseq,
            body_len = text.len(),
        );
        self.conn.send_request(&request).await
    }

    async fn shutdown(&mut self) {
        // Best-effort REGISTER expires=0 to deregister.
        let _ = self.register(0).await;
    }
}

impl SipFrontend {
    /// Two-shot REGISTER: first request gets a 401 with WWW-Authenticate
    /// nonce, second request includes the digest response. RFC 3261 §22.4.
    async fn register(&mut self, expires: u32) -> Result<(), String> {
        // 1. Initial REGISTER.
        let request = self.build_register(expires, None);
        self.conn.send_request(&request).await?;
        let response = self
            .conn
            .read_response_with_timeout(std::time::Duration::from_secs(2))
            .await?;
        if response.contains("SIP/2.0 200") {
            return Ok(()); // server accepted the unchallenged REGISTER
        }
        // 2. Compute digest response from the 401 challenge.
        let (realm, nonce) = parse_challenge(&response)
            .ok_or_else(|| format!("sip register: unexpected response: {response}"))?;
        let auth = digest_response(
            &self.username(),
            &realm,
            &self.password,
            "REGISTER",
            &self.register_uri(),
            &nonce,
        );
        let challenged = self.build_register(expires, Some(&auth));
        self.conn.send_request(&challenged).await?;
        let final_resp = self
            .conn
            .read_response_with_timeout(std::time::Duration::from_secs(2))
            .await?;
        if !final_resp.contains("SIP/2.0 200") {
            return Err(format!("sip register failed: {final_resp}"));
        }
        Ok(())
    }

    fn build_register(&mut self, expires: u32, authorization: Option<&str>) -> String {
        self.cseq = self.cseq.wrapping_add(1);
        let call_id = format!("{}-reg-{}", self.call_id_seed, self.cseq);
        let request_uri = self.register_uri();
        let from = format!("<{}>", self.agent_uri);
        let auth_line = authorization
            .map(|a| format!("Authorization: {a}\r\n"))
            .unwrap_or_default();
        format!(
            "REGISTER {request_uri} SIP/2.0\r\n\
             Via: SIP/2.0/TCP {realm};branch=z9hG4bK-reg-{cseq}\r\n\
             Max-Forwards: 70\r\n\
             From: {from};tag={cseq}\r\n\
             To: {from}\r\n\
             Call-ID: {call_id}\r\n\
             CSeq: {cseq} REGISTER\r\n\
             Contact: {from};expires={expires}\r\n\
             User-Agent: harmonia-sip/0.1\r\n\
             {auth_line}\
             Content-Length: 0\r\n\
             \r\n",
            realm = self.realm_default,
            cseq = self.cseq,
        )
    }

    fn register_uri(&self) -> String {
        format!("sip:{}", self.realm_default)
    }

    fn username(&self) -> String {
        // agent_uri looks like `sip:<fingerprint>@<realm>`; pull the user part.
        self.agent_uri
            .strip_prefix("sip:")
            .and_then(|rest| rest.split('@').next())
            .unwrap_or(&self.agent_uri)
            .to_string()
    }
}

/// RFC 2617 / RFC 3261 §22.4 MD5 digest auth response.
///
/// `HA1 = MD5(username:realm:password)`,
/// `HA2 = MD5(method:digestURI)`,
/// `response = MD5(HA1:nonce:HA2)`.
fn digest_response(
    username: &str,
    realm: &str,
    password: &str,
    method: &str,
    uri: &str,
    nonce: &str,
) -> String {
    let ha1 = md5_hex(&format!("{username}:{realm}:{password}"));
    let ha2 = md5_hex(&format!("{method}:{uri}"));
    let response = md5_hex(&format!("{ha1}:{nonce}:{ha2}"));
    format!(
        "Digest username=\"{username}\", realm=\"{realm}\", nonce=\"{nonce}\", uri=\"{uri}\", response=\"{response}\""
    )
}

fn md5_hex(input: &str) -> String {
    let mut h = Md5::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}
