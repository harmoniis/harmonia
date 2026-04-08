//! Encryption: key derivation, AEAD (AES-256-GCM), and legacy XOR decryption.

use std::env;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OpenFlags};
use sha2::Sha256;

pub(super) const AEAD_PREFIX: &str = "aead:v1:";
pub(super) const LEGACY_XOR_PREFIX: &str = "enc:";
const KEY_DERIVE_SALT: &[u8] = b"harmonia-vault:key-derivation:v1";
pub(crate) const SCOPED_DERIVE_SALT: &[u8] = b"harmonia-vault:scoped-derivation:v1";

pub(crate) fn parse_key_material(raw: &str) -> Result<Vec<u8>, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("empty key material".to_string());
    }
    if let Some(hex_part) = trimmed.strip_prefix("hex:") {
        return hex::decode(hex_part.trim()).map_err(|e| format!("invalid hex key material: {e}"));
    }
    if trimmed.len() % 2 == 0 && trimmed.chars().all(|c| c.is_ascii_hexdigit()) {
        if let Ok(decoded) = hex::decode(trimmed) {
            if !decoded.is_empty() {
                return Ok(decoded);
            }
        }
    }
    Ok(trimmed.as_bytes().to_vec())
}

pub(crate) fn derive_32(material: &[u8], info: &[u8], salt: &[u8]) -> Result<[u8; 32], String> {
    if material.is_empty() {
        return Err("empty key derivation material".to_string());
    }
    let hk = Hkdf::<Sha256>::new(Some(salt), material);
    let mut out = [0u8; 32];
    hk.expand(info, &mut out)
        .map_err(|_| "hkdf expand failed".to_string())?;
    Ok(out)
}

fn key_from_master_env() -> Result<Option<[u8; 32]>, String> {
    let raw = match env::var("HARMONIA_VAULT_MASTER_KEY") {
        Ok(v) => v,
        Err(_) => return Ok(None),
    };
    let material = parse_key_material(&raw)?;
    let key = derive_32(&material, b"env-master", KEY_DERIVE_SALT)?;
    Ok(Some(key))
}

fn default_wallet_db_path() -> Option<PathBuf> {
    let home = env::var("HOME").ok()?;
    Some(
        PathBuf::from(&home)
            .join(".harmoniis")
            .join("wallet")
            .join("master.db"),
    )
}

fn wallet_db_path() -> Option<PathBuf> {
    if let Ok(v) = env::var("HARMONIA_WALLET_ROOT") {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return Some(PathBuf::from(trimmed).join("master.db"));
        }
    }
    for key in [
        "HARMONIA_VAULT_WALLET_DB",
        "HARMONIA_WALLET_DB",
        "HARMONIIS_WALLET_DB",
    ] {
        if let Ok(v) = env::var(key) {
            let p = PathBuf::from(v);
            if p.exists() {
                return Some(p);
            }
        }
    }
    default_wallet_db_path()
}

fn vault_slot_family() -> String {
    env::var("HARMONIA_VAULT_SLOT_FAMILY")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "vault".to_string())
}

fn key_from_wallet_slot(path: &Path) -> Result<Option<[u8; 32]>, String> {
    let conn = Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| format!("wallet db open failed: {e}"))?;

    let configured = vault_slot_family();
    let mut families = vec![
        configured.clone(),
        "vault".to_string(),
        "harmonia-vault".to_string(),
    ];
    families.sort();
    families.dedup();

    for family in families {
        let slot_hex: Option<String> = conn
            .query_row(
                "SELECT descriptor FROM wallet_slots WHERE family=?1 AND slot_index=0 LIMIT 1",
                params![family],
                |row| row.get(0),
            )
            .ok();

        if let Some(slot_hex) = slot_hex {
            let material = parse_key_material(&slot_hex)?;
            let info = format!("wallet-slot:{family}");
            let key = derive_32(&material, info.as_bytes(), KEY_DERIVE_SALT)?;
            return Ok(Some(key));
        }
    }

    let legacy_root: Option<String> = conn
        .query_row(
            "SELECT value FROM wallet_metadata WHERE key='root_private_key_hex' LIMIT 1",
            [],
            |row| row.get(0),
        )
        .ok();

    if let Some(root_hex) = legacy_root {
        let material = parse_key_material(&root_hex)?;
        let key = derive_32(&material, b"wallet-legacy-root", KEY_DERIVE_SALT)?;
        return Ok(Some(key));
    }

    Ok(None)
}

pub(crate) fn resolve_encryption_key() -> Result<Option<[u8; 32]>, String> {
    if let Some(path) = wallet_db_path() {
        if let Some(from_wallet) = key_from_wallet_slot(&path)? {
            return Ok(Some(from_wallet));
        }
    }
    // Fallback path for explicit override/recovery when wallet DB is unavailable.
    if let Some(from_env) = key_from_master_env()? {
        return Ok(Some(from_env));
    }
    Ok(None)
}

fn legacy_xor_key_material() -> Option<Vec<u8>> {
    env::var("HARMONIA_VAULT_MASTER_KEY")
        .ok()
        .and_then(|v| parse_key_material(&v).ok())
        .filter(|v| !v.is_empty())
}

pub(super) fn encrypt_aead(plaintext: &str, key: &[u8; 32]) -> Result<String, String> {
    let cipher =
        Aes256Gcm::new_from_slice(key).map_err(|e| format!("vault cipher init failed: {e}"))?;
    let mut nonce_bytes = [0u8; 12];
    // Master/root key material is deterministic from wallet slot.
    // Nonce must still be unique per ciphertext; random 96-bit nonce is standard for AES-GCM.
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|_| "vault encryption failed".to_string())?;
    Ok(format!(
        "{}{}:{}",
        AEAD_PREFIX,
        hex::encode(nonce_bytes),
        hex::encode(ciphertext)
    ))
}

fn decrypt_aead(stored: &str, key: &[u8; 32]) -> Option<String> {
    let body = stored.strip_prefix(AEAD_PREFIX)?;
    let (nonce_hex, ct_hex) = body.split_once(':')?;
    let nonce = hex::decode(nonce_hex).ok()?;
    if nonce.len() != 12 {
        return None;
    }
    let ciphertext = hex::decode(ct_hex).ok()?;
    let cipher = Aes256Gcm::new_from_slice(key).ok()?;
    let plaintext = cipher
        .decrypt(Nonce::from_slice(&nonce), ciphertext.as_ref())
        .ok()?;
    String::from_utf8(plaintext).ok()
}

fn decrypt_legacy_xor(stored: &str) -> Option<String> {
    let hex_data = stored.strip_prefix(LEGACY_XOR_PREFIX)?;
    let key = legacy_xor_key_material()?;
    let encrypted = hex::decode(hex_data).ok()?;
    let decrypted: Vec<u8> = encrypted
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ key[i % key.len()])
        .collect();
    String::from_utf8(decrypted).ok()
}

pub(super) fn decrypt_value(stored: &str, key: Option<&[u8; 32]>) -> Option<String> {
    if stored.starts_with(AEAD_PREFIX) {
        let k = key?;
        return decrypt_aead(stored, k);
    }
    if stored.starts_with(LEGACY_XOR_PREFIX) {
        return decrypt_legacy_xor(stored);
    }
    Some(stored.to_string())
}

pub(super) fn allow_unencrypted_writes() -> bool {
    super::db::bool_env("HARMONIA_VAULT_ALLOW_UNENCRYPTED", false)
}
