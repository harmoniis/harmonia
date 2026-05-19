//! Shared PGP trust-store + signer for the Harmonia transport frontends.
//!
//! Every frontend that needs to verify or produce PGP-signed payloads
//! (MQTT, Email, SIP, HTTP/3) talks to this single crate so the keyring,
//! signing key, and on-disk bundle path are owned in one place. Two
//! actor entry points:
//!
//! * [`TrustStoreActor`] — owns the keyring `HashMap<fingerprint, key>`,
//!   loads the bootstrap bundle from `<state>/trust-bundle.json` on
//!   pre_start, exposes [`TrustStoreMsg::Verify`] / `AddKey` / `RemoveKey`.
//! * [`SignerActor`] — owns the agent's secret key + passphrase, exposes
//!   [`SignerMsg::SignDetached`] returning ASCII-armored detached
//!   signatures.
//!
//! Both actors are spawned by the runtime at boot from `runtime::spawn`
//! and the `ActorRef` is stashed in the per-frontend registry alongside
//! the connection actor.

mod signer;
mod trust_store;

pub use signer::{spawn_signer, SignerActor, SignerMsg};
pub use trust_store::{
    spawn_trust_store, TrustStoreActor, TrustStoreEntry, TrustStoreMsg, VerifyOutcome,
};

/// Canonical hash for trust-store fingerprints — uppercase hex of the
/// PGP fingerprint bytes, matching what the marketplace `IdentityService`
/// records on `HarmoniaPushDevice.device_public_key`.
pub fn normalize_fingerprint(fp: &str) -> String {
    fp.replace([' ', ':', '-'], "").to_uppercase()
}

/// Per-frontend handle bundling both PGP actors. Cheap to clone (two
/// `ActorRef`s in `Option`s); each transport frontend that needs PGP
/// receives one of these on init from the runtime spawn path.
#[derive(Clone, Default)]
pub struct TransportPgp {
    pub trust_store: Option<ractor::ActorRef<TrustStoreMsg>>,
    pub signer: Option<ractor::ActorRef<SignerMsg>>,
}

impl TransportPgp {
    pub fn new(
        trust_store: Option<ractor::ActorRef<TrustStoreMsg>>,
        signer: Option<ractor::ActorRef<SignerMsg>>,
    ) -> Self {
        Self {
            trust_store,
            signer,
        }
    }

    /// Verify a detached signature over `data`. When the trust-store actor
    /// isn't running (no bundle, no keys) the call resolves to
    /// [`VerifyOutcome::Unsigned`] so callers can apply the same "no PGP
    /// configured" branch they would for an unsigned input.
    pub async fn verify_detached(&self, data: Vec<u8>, armored_sig: String) -> VerifyOutcome {
        let Some(actor) = &self.trust_store else {
            return VerifyOutcome::Unsigned;
        };
        match ractor::call_t!(
            actor,
            TrustStoreMsg::VerifyDetached,
            5_000,
            data,
            armored_sig
        ) {
            Ok(outcome) => outcome,
            Err(e) => VerifyOutcome::Error(format!("verify rpc: {e}")),
        }
    }

    /// Produce an ASCII-armored detached signature over `data`. Returns
    /// `None` when no signer is configured — the caller's contract is
    /// that an unsigned outbound payload is still valid (mTLS + topic ACL
    /// remain in force).
    pub async fn sign_detached(&self, data: Vec<u8>) -> Option<String> {
        let actor = self.signer.as_ref()?;
        match ractor::call_t!(actor, SignerMsg::SignDetached, 5_000, data) {
            Ok(Ok(armored)) => Some(armored),
            _ => None,
        }
    }
}
