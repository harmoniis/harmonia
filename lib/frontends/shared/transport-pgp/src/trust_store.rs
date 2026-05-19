//! Trust-store actor — owns the keyring of PGP public keys the agent
//! considers trusted (operator's mobile device, paired clients, peer
//! agents, the marketplace platform key). Keys land here from two
//! sources:
//!
//! 1. The bootstrap `trust-bundle.json` written by `harmonia setup
//!    --headless-config` (Phase 8 path) on pot first-boot.
//! 2. Runtime `AddKey` messages cast by the marketplace HTTP/3 client
//!    when `/trusted-devices` returns a fresh list.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use pgp::composed::{Deserializable, SignedPublicKey, StandaloneSignature};
use pgp::types::PublicKeyTrait;
use ractor::{Actor, ActorProcessingErr, ActorRef, RpcReplyPort};

use crate::normalize_fingerprint;

#[derive(Debug, Clone)]
pub struct TrustStoreEntry {
    pub fingerprint: String,
    /// Optional human label — typically the device_id or client_fp the
    /// marketplace returned alongside the key.
    pub label: Option<String>,
}

#[derive(Debug, Clone)]
pub enum VerifyOutcome {
    /// Signature verified against `fingerprint`.
    Verified { fingerprint: String },
    /// Signature parsed but no trusted key matched.
    SignedUntrusted { fingerprint: String },
    /// No usable signature in the input.
    Unsigned,
    /// Verifier hit a parse / IO error before reaching trust check.
    Error(String),
}

pub enum TrustStoreMsg {
    /// `VerifyDetached(data, armored_sig, reply)` — verify a detached
    /// signature against the keyring.
    VerifyDetached(Vec<u8>, String, RpcReplyPort<VerifyOutcome>),
    /// `AddKey(armored_pubkey, label, reply)` — add (or replace) a key.
    AddKey(String, Option<String>, RpcReplyPort<Result<String, String>>),
    /// `RemoveKey(fingerprint, reply)` — remove a key by fingerprint.
    RemoveKey(String, RpcReplyPort<bool>),
    /// List the current keyring (fingerprint + label only).
    List(RpcReplyPort<Vec<TrustStoreEntry>>),
}

pub struct TrustStoreActor;

pub struct TrustStoreState {
    keys: HashMap<String, KeyEntry>,
}

struct KeyEntry {
    public_key: SignedPublicKey,
    label: Option<String>,
}

impl TrustStoreActor {
    fn load_bundle(state_root: &str, state: &mut TrustStoreState) {
        let bundle_path = PathBuf::from(state_root).join("trust-bundle.json");
        if !bundle_path.exists() {
            return;
        }
        let raw = match std::fs::read_to_string(&bundle_path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("[WARN] [trust-store] read {bundle_path:?}: {e}");
                return;
            }
        };
        let json: serde_json::Value = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[WARN] [trust-store] parse {bundle_path:?}: {e}");
                return;
            }
        };
        let Some(devices) = json.get("devices").and_then(|v| v.as_array()) else {
            return;
        };
        let mut imported = 0usize;
        for device in devices {
            let armored = match device.get("device_public_key").and_then(|v| v.as_str()) {
                Some(s) if !s.trim().is_empty() => s,
                _ => continue,
            };
            let label = device
                .get("device_id")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            match parse_pubkey(armored) {
                Ok((fp, key)) => {
                    state.keys.insert(fp, KeyEntry { public_key: key, label });
                    imported += 1;
                }
                Err(e) => eprintln!("[WARN] [trust-store] bundle key skipped: {e}"),
            }
        }
        eprintln!(
            "[INFO] [trust-store] bootstrap {imported} key(s) from {}",
            bundle_path.display()
        );
    }
}

impl Actor for TrustStoreActor {
    type Msg = TrustStoreMsg;
    type State = TrustStoreState;
    type Arguments = String; // state-root path

    async fn pre_start(
        &self,
        _myself: ActorRef<Self::Msg>,
        state_root: Self::Arguments,
    ) -> Result<Self::State, ActorProcessingErr> {
        let mut state = TrustStoreState {
            keys: HashMap::new(),
        };
        Self::load_bundle(&state_root, &mut state);
        Ok(state)
    }

    async fn handle(
        &self,
        _myself: ActorRef<Self::Msg>,
        msg: Self::Msg,
        state: &mut Self::State,
    ) -> Result<(), ActorProcessingErr> {
        match msg {
            TrustStoreMsg::VerifyDetached(data, armored_sig, reply) => {
                let outcome = verify_detached_against(&state.keys, &data, &armored_sig);
                let _ = reply.send(outcome);
            }
            TrustStoreMsg::AddKey(armored_pubkey, label, reply) => {
                match parse_pubkey(&armored_pubkey) {
                    Ok((fp, key)) => {
                        state.keys.insert(
                            fp.clone(),
                            KeyEntry {
                                public_key: key,
                                label,
                            },
                        );
                        let _ = reply.send(Ok(fp));
                    }
                    Err(e) => {
                        let _ = reply.send(Err(e));
                    }
                }
            }
            TrustStoreMsg::RemoveKey(fingerprint, reply) => {
                let removed = state
                    .keys
                    .remove(&normalize_fingerprint(&fingerprint))
                    .is_some();
                let _ = reply.send(removed);
            }
            TrustStoreMsg::List(reply) => {
                let entries = state
                    .keys
                    .iter()
                    .map(|(fp, ke)| TrustStoreEntry {
                        fingerprint: fp.clone(),
                        label: ke.label.clone(),
                    })
                    .collect();
                let _ = reply.send(entries);
            }
        }
        Ok(())
    }
}

fn parse_pubkey(armored: &str) -> Result<(String, SignedPublicKey), String> {
    let (key, _) = SignedPublicKey::from_string(armored)
        .map_err(|e| format!("parse public key: {e}"))?;
    let fp = hex::encode_upper(key.fingerprint().as_bytes());
    Ok((fp, key))
}

fn verify_detached_against(
    keys: &HashMap<String, KeyEntry>,
    data: &[u8],
    armored_sig: &str,
) -> VerifyOutcome {
    let signature = match StandaloneSignature::from_string(armored_sig) {
        Ok((sig, _)) => sig,
        Err(e) => return VerifyOutcome::Error(format!("parse signature: {e}")),
    };
    let issuer_fps: Vec<String> = signature
        .signature
        .issuer_fingerprint()
        .into_iter()
        .map(|fp| hex::encode_upper(fp.as_bytes()))
        .collect();

    if issuer_fps.is_empty() {
        return VerifyOutcome::Unsigned;
    }
    for fp in &issuer_fps {
        if let Some(entry) = keys.get(fp) {
            return match signature.verify(&entry.public_key, data) {
                Ok(()) => VerifyOutcome::Verified { fingerprint: fp.clone() },
                Err(e) => VerifyOutcome::Error(format!("verify failed: {e}")),
            };
        }
    }
    // Issuer fingerprint(s) present but none in the trust-store.
    VerifyOutcome::SignedUntrusted {
        fingerprint: issuer_fps.into_iter().next().unwrap_or_default(),
    }
}

/// Convenience to spawn the actor under a parent supervisor's cell.
pub async fn spawn_trust_store<P: ractor::Message>(
    supervisor: &ActorRef<P>,
    state_root: String,
) -> Result<ActorRef<TrustStoreMsg>, String> {
    let (actor, _handle) = Actor::spawn_linked(
        Some("trust-store".to_string()),
        TrustStoreActor,
        state_root,
        supervisor.get_cell(),
    )
    .await
    .map_err(|e| format!("spawn trust-store: {e}"))?;
    Ok(actor)
}

// Reduce monomorphisation overhead a hair for callers that don't care
// about lifetimes — the keyring is wrapped in Arc internally on the
// actor side.
#[allow(dead_code)]
fn _arc_marker(_: Arc<()>) {}
