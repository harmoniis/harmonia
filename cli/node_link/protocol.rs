use super::*;

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
