//! Admin intent verification using Ed25519 signatures.
//!
//! Owner's public key is stored in vault. Private key stays on the owner's device.
//! Privileged mutations that require admin intent must carry a valid signature.

use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::sync::{OnceLock, RwLock};

use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use harmonia_vault::{get_secret_for_component, init_from_env};

const VERSION: &[u8] = b"harmonia-admin-intent/0.1.0\0";

static LAST_ERROR: OnceLock<RwLock<String>> = OnceLock::new();

fn last_error_lock() -> &'static RwLock<String> {
    LAST_ERROR.get_or_init(|| RwLock::new(String::new()))
}

fn set_error(msg: impl Into<String>) {
    if let Ok(mut slot) = last_error_lock().write() {
        *slot = msg.into();
    }
}

fn clear_error() {
    if let Ok(mut slot) = last_error_lock().write() {
        slot.clear();
    }
}

fn cstr_to_string(ptr: *const c_char) -> Result<String, String> {
    if ptr.is_null() {
        return Err("null pointer".to_string());
    }
    // Safety: caller must pass a valid NUL-terminated C string.
    let c = unsafe { CStr::from_ptr(ptr) };
    Ok(c.to_string_lossy().into_owned())
}

fn to_c_string(value: String) -> *mut c_char {
    match CString::new(value) {
        Ok(v) => v.into_raw(),
        Err(_) => std::ptr::null_mut(),
    }
}

fn decode_pubkey(raw: &str) -> Result<Vec<u8>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty public key".to_string());
    }
    let is_hex = trimmed.chars().all(|c| c.is_ascii_hexdigit());
    if is_hex && trimmed.len() == 64 {
        return hex::decode(trimmed).map_err(|e| format!("invalid hex public key: {e}"));
    }
    let bytes = trimmed.as_bytes().to_vec();
    if bytes.len() == 32 {
        return Ok(bytes);
    }
    Err("public key must be 32 raw bytes or 64-char hex".to_string())
}

/// Verify an admin intent signature.
///
/// The signed message is `action|params` (pipe-separated).
/// `sig_hex` is the hex-encoded Ed25519 signature.
/// `pubkey_bytes` is the 32-byte Ed25519 public key.
pub fn verify_admin_intent(
    action: &str,
    params: &str,
    sig_hex: &str,
    pubkey_bytes: &[u8],
) -> Result<(), String> {
    if pubkey_bytes.len() != 32 {
        return Err(format!(
            "invalid public key length: expected 32, got {}",
            pubkey_bytes.len()
        ));
    }

    let key_bytes: [u8; 32] = pubkey_bytes
        .try_into()
        .map_err(|_| "invalid public key bytes".to_string())?;

    let verifying_key =
        VerifyingKey::from_bytes(&key_bytes).map_err(|e| format!("invalid public key: {}", e))?;

    let sig_bytes = hex::decode(sig_hex).map_err(|e| format!("invalid signature hex: {}", e))?;

    if sig_bytes.len() != 64 {
        return Err(format!(
            "invalid signature length: expected 64, got {}",
            sig_bytes.len()
        ));
    }

    let sig_array: [u8; 64] = sig_bytes
        .try_into()
        .map_err(|_| "invalid signature bytes".to_string())?;

    let signature = Signature::from_bytes(&sig_array);
    let message = format!("{}|{}", action, params);

    verifying_key
        .verify(message.as_bytes(), &signature)
        .map_err(|e| format!("signature verification failed: {}", e))
}

/// Verify admin intent using a public key looked up from vault.
pub fn verify_admin_intent_with_vault(
    action: &str,
    params: &str,
    sig_hex: &str,
    pubkey_symbol: &str,
) -> Result<(), String> {
    init_from_env()?;
    let raw = get_secret_for_component("admin-intent", pubkey_symbol)?
        .ok_or_else(|| format!("missing public key in vault: {}", pubkey_symbol))?;
    let key = decode_pubkey(&raw)?;
    verify_admin_intent(action, params, sig_hex, &key)
}

/// Check if an operation requires admin intent based on the configured list.
pub fn is_admin_intent_op(op: &str, required_ops: &[&str]) -> bool {
    required_ops.iter().any(|r| r.eq_ignore_ascii_case(op))
}

#[no_mangle]
pub extern "C" fn harmonia_admin_intent_version() -> *const c_char {
    VERSION.as_ptr().cast()
}

#[no_mangle]
pub extern "C" fn harmonia_admin_intent_healthcheck() -> i32 {
    1
}

#[no_mangle]
pub extern "C" fn harmonia_admin_intent_init() -> i32 {
    match init_from_env() {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(e);
            -1
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_admin_intent_verify_with_vault(
    action: *const c_char,
    params: *const c_char,
    sig_hex: *const c_char,
    pubkey_symbol: *const c_char,
) -> i32 {
    let action = match cstr_to_string(action) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let params = match cstr_to_string(params) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let sig_hex = match cstr_to_string(sig_hex) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };
    let pubkey_symbol = match cstr_to_string(pubkey_symbol) {
        Ok(v) => v,
        Err(e) => {
            set_error(e);
            return -1;
        }
    };

    match verify_admin_intent_with_vault(&action, &params, &sig_hex, &pubkey_symbol) {
        Ok(()) => {
            clear_error();
            1
        }
        Err(e) => {
            set_error(e);
            0
        }
    }
}

#[no_mangle]
pub extern "C" fn harmonia_admin_intent_last_error() -> *mut c_char {
    to_c_string(
        last_error_lock()
            .read()
            .map(|v| v.clone())
            .unwrap_or_else(|_| "admin-intent lock poisoned".to_string()),
    )
}

#[no_mangle]
pub extern "C" fn harmonia_admin_intent_free_string(ptr: *mut c_char) {
    if ptr.is_null() {
        return;
    }
    // Safety: pointer must come from CString::into_raw in this crate.
    unsafe {
        drop(CString::from_raw(ptr));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    #[test]
    fn verify_valid_signature() {
        let mut rng = rand_core::OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let action = "vault-set";
        let params = "op=vault-set key=test ts=1710000000000";
        let message = format!("{}|{}", action, params);

        let signature = signing_key.sign(message.as_bytes());
        let sig_hex = hex::encode(signature.to_bytes());

        let result = verify_admin_intent(action, params, &sig_hex, verifying_key.as_bytes());
        assert!(result.is_ok());
    }

    #[test]
    fn reject_invalid_signature() {
        let mut rng = rand_core::OsRng;
        let signing_key = SigningKey::generate(&mut rng);
        let verifying_key = signing_key.verifying_key();

        let result = verify_admin_intent(
            "vault-set",
            "op=vault-set key=test ts=1710000000000",
            &"00".repeat(64),
            verifying_key.as_bytes(),
        );
        assert!(result.is_err());
    }

    #[test]
    fn is_admin_intent_op_check() {
        let ops = &[
            "harmony-policy-set",
            "matrix-set-edge",
            "matrix-reset-defaults",
        ];
        assert!(is_admin_intent_op("harmony-policy-set", ops));
        assert!(!is_admin_intent_op("gateway-send", ops));
    }
}
