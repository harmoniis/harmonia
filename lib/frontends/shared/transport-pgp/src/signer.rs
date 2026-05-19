//! Signer actor — owns the agent's PGP secret key + passphrase, exposes a
//! `SignDetached` message returning ASCII-armored detached signatures over
//! caller-supplied bytes. Same actor signs MQTT payloads, SIP X-Harmonia-Sig
//! headers, HTTP/3 PGP-Hello frames, and Email outbound bodies — every
//! transport that needs a producer-side signature.

use pgp::composed::{Deserializable, SignedSecretKey, StandaloneSignature};
use pgp::crypto::hash::HashAlgorithm;
use pgp::packet::{SignatureConfig, SignatureType};
use pgp::types::PublicKeyTrait;
use pgp::ArmorOptions;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

pub enum SignerMsg {
    /// `SignDetached(data, reply)` — produce an ASCII-armored detached
    /// signature over `data`.
    SignDetached(Vec<u8>, RpcReplyPort<Result<String, String>>),
    /// Surface the agent's own PGP fingerprint (uppercase hex).
    Fingerprint(RpcReplyPort<String>),
}

pub struct SignerActor;

pub struct SignerState {
    secret_key: SignedSecretKey,
    passphrase: String,
    fingerprint: String,
}

impl Actor for SignerActor {
    type Msg = SignerMsg;
    type State = SignerState;
    /// `(armored_secret_key, passphrase)` — passphrase is empty for
    /// passphrase-less keys.
    type Arguments = (String, String);

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        (armored, passphrase): Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let (secret_key, _) = SignedSecretKey::from_string(&armored)
            .map_err(|e| ActorProcessingErr::from(format!("signer parse: {e}")))?;
        let fingerprint = hex::encode_upper(secret_key.fingerprint().as_bytes());
        eprintln!("[INFO] [signer] ready (fp={fingerprint})");
        Ok(SignerState {
            secret_key,
            passphrase,
            fingerprint,
        })
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            SignerMsg::SignDetached(data, reply) => {
                let result = sign_detached(&state.secret_key, &state.passphrase, &data);
                let _ = reply.send(result);
            }
            SignerMsg::Fingerprint(reply) => {
                let _ = reply.send(state.fingerprint.clone());
            }
        }
        Ok(())
    }
}

fn sign_detached(
    secret_key: &SignedSecretKey,
    passphrase: &str,
    data: &[u8],
) -> Result<String, String> {
    let config = SignatureConfig::v4(
        SignatureType::Binary,
        secret_key.algorithm(),
        HashAlgorithm::SHA2_256,
    );
    let pass = passphrase.to_string();
    let signature = config
        .sign(secret_key, move || pass.clone(), data)
        .map_err(|e| format!("sign: {e}"))?;
    StandaloneSignature::new(signature)
        .to_armored_string(ArmorOptions::default())
        .map_err(|e| format!("armor: {e}"))
}

/// Spawn a Signer actor under `supervisor`. Returns `None` when no signing
/// key is configured in vault — callers tolerate the missing actor and
/// emit unsigned payloads.
pub async fn spawn_signer<P: ractor::Message>(
    supervisor: &ActorRef<P>,
    armored: String,
    passphrase: String,
) -> Result<ActorRef<SignerMsg>, String> {
    let (actor, _handle) = Actor::spawn_linked(
        Some("transport-signer".to_string()),
        SignerActor,
        (armored, passphrase),
        supervisor.get_cell(),
    )
    .await
    .map_err(|e| format!("spawn signer: {e}"))?;
    Ok(actor)
}
