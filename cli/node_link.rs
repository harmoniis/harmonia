use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use snow::{params::NoiseParams, Builder};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const NODE_LINK_PROTOCOL_VERSION: u8 = 1;

// Pairing bootstrap when peers do not know each other's static keys yet.
pub const NOISE_PAIR_PATTERN: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";

// Reconnect path after pairing persists the remote static identity.
pub const NOISE_SESSION_PATTERN: &str = "Noise_IK_25519_ChaChaPoly_BLAKE2s";

pub const NOISE_IMPLEMENTATION: &str = "snow";

// Current runtime uses Tailscale as the network substrate, not raw WireGuard.
pub const TAILSCALE_TRANSPORT_KIND: &str = "tailscale";

// Preferred management path for production is the existing local Tailscale daemon
// via LocalAPI. Embedded libtailscale remains the fallback for future userspace mode.
pub const TAILSCALE_MANAGEMENT_KIND: &str = "daemon-localapi";
pub const TAILSCALE_EMBED_FALLBACK_KIND: &str = "embedded-libtailscale";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeLinkStack {
    pub protocol_version: u8,
    pub transport_kind: String,
    pub tailscale_management_kind: String,
    pub tailscale_embed_fallback_kind: String,
    pub noise_implementation: String,
    pub noise_pair_pattern: String,
    pub noise_session_pattern: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLinkIdentityRecord {
    pub version: u8,
    pub key_source: String,
    pub created_at_ms: u64,
    pub public_key: String,
    pub private_key: String,
    pub public_key_id: String,
    pub stack: NodeLinkStack,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLinkAdvertisement {
    pub public_key: String,
    pub public_key_id: String,
    pub stack: NodeLinkStack,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RemoteNodeLink {
    pub public_key: String,
    pub public_key_id: String,
    #[serde(default)]
    pub stack: Option<NodeLinkStack>,
}

#[cfg(test)]
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairHandshakeProof {
    pub initiator_remote_key_id: String,
    pub responder_remote_key_id: String,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

pub fn stack() -> NodeLinkStack {
    NodeLinkStack {
        protocol_version: NODE_LINK_PROTOCOL_VERSION,
        transport_kind: TAILSCALE_TRANSPORT_KIND.to_string(),
        tailscale_management_kind: TAILSCALE_MANAGEMENT_KIND.to_string(),
        tailscale_embed_fallback_kind: TAILSCALE_EMBED_FALLBACK_KIND.to_string(),
        noise_implementation: NOISE_IMPLEMENTATION.to_string(),
        noise_pair_pattern: NOISE_PAIR_PATTERN.to_string(),
        noise_session_pattern: NOISE_SESSION_PATTERN.to_string(),
    }
}

fn pair_params() -> Result<NoiseParams, Box<dyn std::error::Error>> {
    Ok(NOISE_PAIR_PATTERN.parse()?)
}

#[cfg(test)]
fn session_params() -> Result<NoiseParams, Box<dyn std::error::Error>> {
    Ok(NOISE_SESSION_PATTERN.parse()?)
}

fn identity_path(node: &crate::paths::NodeIdentity) -> Result<PathBuf, Box<dyn std::error::Error>> {
    Ok(crate::paths::node_dir(&node.label)?.join("node-link.json"))
}

fn encode_key(bytes: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(test)]
fn decode_key(raw: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(URL_SAFE_NO_PAD.decode(raw.trim())?)
}

fn public_key_id_from_b64(public_key: &str) -> String {
    public_key.chars().take(16).collect()
}

fn write_identity(
    path: &Path,
    identity: &NodeLinkIdentityRecord,
) -> Result<(), Box<dyn std::error::Error>> {
    let json = serde_json::to_string_pretty(identity)?;
    fs::write(path, format!("{json}\n"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let permissions = fs::Permissions::from_mode(0o600);
        let _ = fs::set_permissions(path, permissions);
    }
    Ok(())
}

pub fn load_or_create_identity(
    node: &crate::paths::NodeIdentity,
) -> Result<NodeLinkIdentityRecord, Box<dyn std::error::Error>> {
    crate::paths::ensure_node_layout(node)?;
    let path = identity_path(node)?;
    if path.exists() {
        let raw = fs::read_to_string(path)?;
        return Ok(serde_json::from_str(&raw)?);
    }

    let builder = Builder::new(pair_params()?);
    let keypair = builder.generate_keypair()?;
    let public_key = encode_key(&keypair.public);
    let identity = NodeLinkIdentityRecord {
        version: NODE_LINK_PROTOCOL_VERSION,
        key_source: "node-file".to_string(),
        created_at_ms: now_ms(),
        public_key_id: public_key_id_from_b64(&public_key),
        public_key,
        private_key: encode_key(&keypair.private),
        stack: stack(),
    };
    write_identity(&path, &identity)?;
    Ok(identity)
}

pub fn advertise_identity(
    node: &crate::paths::NodeIdentity,
) -> Result<NodeLinkAdvertisement, Box<dyn std::error::Error>> {
    let identity = load_or_create_identity(node)?;
    Ok(NodeLinkAdvertisement {
        public_key: identity.public_key,
        public_key_id: identity.public_key_id,
        stack: identity.stack,
    })
}

pub fn remote_from_advert(advertisement: &NodeLinkAdvertisement) -> RemoteNodeLink {
    RemoteNodeLink {
        public_key: advertisement.public_key.clone(),
        public_key_id: advertisement.public_key_id.clone(),
        stack: Some(advertisement.stack.clone()),
    }
}

pub fn ensure_compatible_stack(
    advertisement: &NodeLinkAdvertisement,
) -> Result<(), Box<dyn std::error::Error>> {
    let local = stack();
    let remote = &advertisement.stack;
    if remote.protocol_version != local.protocol_version {
        return Err(format!(
            "unsupported node-link protocol version: local={} remote={}",
            local.protocol_version, remote.protocol_version
        )
        .into());
    }
    if remote.transport_kind != local.transport_kind {
        return Err(format!(
            "unsupported node-link transport: local={} remote={}",
            local.transport_kind, remote.transport_kind
        )
        .into());
    }
    if remote.noise_implementation != local.noise_implementation
        || remote.noise_pair_pattern != local.noise_pair_pattern
        || remote.noise_session_pattern != local.noise_session_pattern
    {
        return Err("node-link Noise stack is incompatible".into());
    }
    Ok(())
}

#[cfg(test)]
pub fn prove_pair_handshake(
    initiator: &NodeLinkIdentityRecord,
    responder: &NodeLinkIdentityRecord,
) -> Result<PairHandshakeProof, Box<dyn std::error::Error>> {
    let init_private = decode_key(&initiator.private_key)?;
    let resp_private = decode_key(&responder.private_key)?;
    let params_i = pair_params()?;
    let params_r = pair_params()?;
    let mut init = Builder::new(params_i)
        .local_private_key(&init_private)?
        .build_initiator()?;
    let mut resp = Builder::new(params_r)
        .local_private_key(&resp_private)?
        .build_responder()?;

    let mut msg1 = [0u8; 512];
    let mut msg2 = [0u8; 512];
    let mut msg3 = [0u8; 512];
    let mut scratch = [0u8; 512];

    let len1 = init.write_message(&[], &mut msg1)?;
    resp.read_message(&msg1[..len1], &mut scratch)?;

    let len2 = resp.write_message(&[], &mut msg2)?;
    init.read_message(&msg2[..len2], &mut scratch)?;

    let len3 = init.write_message(&[], &mut msg3)?;
    resp.read_message(&msg3[..len3], &mut scratch)?;

    let mut init_transport = init.into_transport_mode()?;
    let mut resp_transport = resp.into_transport_mode()?;

    let init_remote = encode_key(
        init_transport
            .get_remote_static()
            .ok_or("initiator transport missing remote static")?,
    );
    let resp_remote = encode_key(
        resp_transport
            .get_remote_static()
            .ok_or("responder transport missing remote static")?,
    );

    let mut ciphertext = [0u8; 512];
    let mut plaintext = [0u8; 512];
    let encrypted_len = init_transport.write_message(b"pair-proof", &mut ciphertext)?;
    let decrypted_len =
        resp_transport.read_message(&ciphertext[..encrypted_len], &mut plaintext)?;
    if &plaintext[..decrypted_len] != b"pair-proof" {
        return Err("pair handshake transport proof failed".into());
    }

    Ok(PairHandshakeProof {
        initiator_remote_key_id: public_key_id_from_b64(&init_remote),
        responder_remote_key_id: public_key_id_from_b64(&resp_remote),
    })
}

#[cfg(test)]
pub fn prove_session_resume(
    initiator: &NodeLinkIdentityRecord,
    responder: &NodeLinkIdentityRecord,
) -> Result<PairHandshakeProof, Box<dyn std::error::Error>> {
    let init_private = decode_key(&initiator.private_key)?;
    let init_public = decode_key(&initiator.public_key)?;
    let resp_private = decode_key(&responder.private_key)?;
    let resp_public = decode_key(&responder.public_key)?;
    let params_i = session_params()?;
    let params_r = session_params()?;
    let mut init = Builder::new(params_i)
        .local_private_key(&init_private)?
        .remote_public_key(&resp_public)?
        .build_initiator()?;
    let mut resp = Builder::new(params_r)
        .local_private_key(&resp_private)?
        .remote_public_key(&init_public)?
        .build_responder()?;

    let mut msg1 = [0u8; 512];
    let mut msg2 = [0u8; 512];
    let mut scratch = [0u8; 512];

    let len1 = init.write_message(&[], &mut msg1)?;
    resp.read_message(&msg1[..len1], &mut scratch)?;

    let len2 = resp.write_message(&[], &mut msg2)?;
    init.read_message(&msg2[..len2], &mut scratch)?;

    let mut init_transport = init.into_transport_mode()?;
    let mut resp_transport = resp.into_transport_mode()?;

    let init_remote = encode_key(
        init_transport
            .get_remote_static()
            .ok_or("initiator transport missing remote static")?,
    );
    let resp_remote = encode_key(
        resp_transport
            .get_remote_static()
            .ok_or("responder transport missing remote static")?,
    );

    let mut ciphertext = [0u8; 512];
    let mut plaintext = [0u8; 512];
    let encrypted_len = init_transport.write_message(b"session-proof", &mut ciphertext)?;
    let decrypted_len =
        resp_transport.read_message(&ciphertext[..encrypted_len], &mut plaintext)?;
    if &plaintext[..decrypted_len] != b"session-proof" {
        return Err("session resume transport proof failed".into());
    }

    Ok(PairHandshakeProof {
        initiator_remote_key_id: public_key_id_from_b64(&init_remote),
        responder_remote_key_id: public_key_id_from_b64(&resp_remote),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::{InstallProfile, NodeIdentity, NodeRole};
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    const TEST_ENV_KEYS: &[&str] = &["HARMONIA_DATA_DIR", "HARMONIA_STATE_ROOT"];

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn acquire_env_lock() -> std::sync::MutexGuard<'static, ()> {
        match env_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
    }

    struct EnvSnapshot(Vec<(&'static str, Option<OsString>)>);

    impl EnvSnapshot {
        fn capture(keys: &[&'static str]) -> Self {
            Self(
                keys.iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect::<Vec<_>>(),
            )
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                if let Some(saved) = value {
                    std::env::set_var(key, saved);
                } else {
                    std::env::remove_var(key);
                }
            }
        }
    }

    fn test_node(label: &str) -> NodeIdentity {
        NodeIdentity {
            label: label.to_string(),
            hostname: label.to_string(),
            role: NodeRole::Agent,
            install_profile: InstallProfile::FullAgent,
        }
    }

    #[test]
    fn node_link_identity_is_stable_on_disk() {
        let _guard = acquire_env_lock();
        let _snapshot = EnvSnapshot::capture(TEST_ENV_KEYS);
        let root = std::env::temp_dir().join(format!("harmonia-node-link-{}", now_ms()));
        fs::create_dir_all(&root).expect("temp root");
        std::env::set_var("HARMONIA_DATA_DIR", &root);
        std::env::set_var("HARMONIA_STATE_ROOT", &root);

        let node = test_node("node-a");
        let first = load_or_create_identity(&node).expect("create identity");
        let second = load_or_create_identity(&node).expect("reload identity");
        assert_eq!(first.public_key, second.public_key);
        assert_eq!(first.public_key_id, second.public_key_id);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn xx_pair_handshake_and_ik_resume_both_work() {
        let _guard = acquire_env_lock();
        let _snapshot = EnvSnapshot::capture(TEST_ENV_KEYS);
        let root = std::env::temp_dir().join(format!("harmonia-node-link-{}", now_ms()));
        fs::create_dir_all(&root).expect("temp root");
        std::env::set_var("HARMONIA_DATA_DIR", &root);
        std::env::set_var("HARMONIA_STATE_ROOT", &root);

        let initiator = load_or_create_identity(&test_node("node-a")).expect("identity a");
        let responder = load_or_create_identity(&test_node("node-b")).expect("identity b");

        let pair = prove_pair_handshake(&initiator, &responder).expect("pair handshake");
        assert_eq!(pair.initiator_remote_key_id, responder.public_key_id);
        assert_eq!(pair.responder_remote_key_id, initiator.public_key_id);

        let resume = prove_session_resume(&initiator, &responder).expect("resume handshake");
        assert_eq!(resume.initiator_remote_key_id, responder.public_key_id);
        assert_eq!(resume.responder_remote_key_id, initiator.public_key_id);

        let _ = fs::remove_dir_all(root);
    }
}
