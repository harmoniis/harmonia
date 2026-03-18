use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use aes_gcm::aead::{Aead, KeyInit};
use aes_gcm::{Aes256Gcm, Nonce};
use hkdf::Hkdf;
use rand_core::{OsRng, RngCore};
use rusqlite::{params, Connection, OpenFlags};
use sha2::Sha256;

const AEAD_PREFIX: &str = "aead:v1:";
const LEGACY_XOR_PREFIX: &str = "enc:";
const KEY_DERIVE_SALT: &[u8] = b"harmonia-vault:key-derivation:v1";
const SCOPED_DERIVE_SALT: &[u8] = b"harmonia-vault:scoped-derivation:v1";

fn bool_env(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(v) => matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(_) => default,
    }
}

fn allow_unencrypted_writes() -> bool {
    bool_env("HARMONIA_VAULT_ALLOW_UNENCRYPTED", false)
}

fn vault_slot_family() -> String {
    env::var("HARMONIA_VAULT_SLOT_FAMILY")
        .ok()
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "vault".to_string())
}

fn parse_key_material(raw: &str) -> Result<Vec<u8>, String> {
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

fn derive_32(material: &[u8], info: &[u8], salt: &[u8]) -> Result<[u8; 32], String> {
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

fn resolve_encryption_key() -> Result<Option<[u8; 32]>, String> {
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

fn encrypt_aead(plaintext: &str, key: &[u8; 32]) -> Result<String, String> {
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

fn decrypt_value(stored: &str, key: Option<&[u8; 32]>) -> Option<String> {
    if stored.starts_with(AEAD_PREFIX) {
        let k = key?;
        return decrypt_aead(stored, k);
    }
    if stored.starts_with(LEGACY_XOR_PREFIX) {
        return decrypt_legacy_xor(stored);
    }
    Some(stored.to_string())
}

fn state_root() -> PathBuf {
    env::var("HARMONIA_STATE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| env::temp_dir().join("harmonia"))
}

pub fn store_path() -> PathBuf {
    env::var("HARMONIA_VAULT_DB")
        .or_else(|_| env::var("HARMONIA_VAULT_PATH"))
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_root().join("vault.db"))
}

pub fn normalize_symbol(symbol: &str) -> String {
    symbol
        .trim()
        .trim_start_matches(':')
        .to_ascii_lowercase()
        .replace('_', "-")
}

pub fn normalize_env_symbol(raw: &str) -> String {
    normalize_symbol(&raw.to_ascii_lowercase().replace("__", "-"))
}

fn open_db() -> Result<Connection, String> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("vault db dir create failed: {e}"))?;
    }
    let conn = Connection::open(path).map_err(|e| format!("vault db open failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS secrets (
            symbol TEXT PRIMARY KEY NOT NULL,
            value TEXT NOT NULL
        )",
        [],
    )
    .map_err(|e| format!("vault schema init failed: {e}"))?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS vault_audit (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            ts INTEGER NOT NULL DEFAULT (strftime('%s','now')),
            op TEXT NOT NULL,
            symbol TEXT NOT NULL,
            source TEXT DEFAULT ''
        )",
        [],
    )
    .map_err(|e| format!("vault audit schema init failed: {e}"))?;
    Ok(conn)
}

pub fn load_store_file() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let conn = match open_db() {
        Ok(v) => v,
        Err(_) => return map,
    };
    let key = resolve_encryption_key().ok().flatten();

    let mut stmt = match conn.prepare("SELECT symbol, value FROM secrets") {
        Ok(v) => v,
        Err(_) => return map,
    };
    let rows = match stmt.query_map([], |row| {
        let symbol: String = row.get(0)?;
        let value: String = row.get(1)?;
        Ok((symbol, value))
    }) {
        Ok(v) => v,
        Err(_) => return map,
    };
    for (symbol, value) in rows.flatten() {
        if let Some(plain) = decrypt_value(&value, key.as_ref()) {
            map.insert(normalize_symbol(&symbol), plain);
        }
    }
    map
}

pub fn upsert_secret(symbol: &str, value: &str) -> Result<(), String> {
    let conn = open_db()?;
    let norm = normalize_symbol(symbol);
    let key = resolve_encryption_key()?;

    let stored = match key {
        Some(ref k) => encrypt_aead(value, k)?,
        None if allow_unencrypted_writes() => value.to_string(),
        None => return Err(
            "vault encryption key unavailable: configure wallet slot or HARMONIA_VAULT_MASTER_KEY"
                .to_string(),
        ),
    };

    conn.execute(
        "INSERT INTO secrets(symbol, value) VALUES (?1, ?2)
         ON CONFLICT(symbol) DO UPDATE SET value=excluded.value",
        params![norm, stored],
    )
    .map_err(|e| format!("vault upsert failed: {e}"))?;

    let _ = conn.execute(
        "INSERT INTO vault_audit(op, symbol) VALUES ('set', ?1)",
        params![norm],
    );
    Ok(())
}

pub fn derive_scoped_secret_hex(scope: &str) -> Result<String, String> {
    let scope = scope.trim();
    if scope.is_empty() {
        return Err("scope cannot be empty".to_string());
    }
    let key = resolve_encryption_key()?
        .ok_or_else(|| "vault key unavailable for scoped derivation".to_string())?;
    let derived = derive_32(&key, scope.as_bytes(), SCOPED_DERIVE_SALT)?;
    Ok(hex::encode(derived))
}

pub fn list_symbols() -> Result<Vec<String>, String> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT symbol FROM secrets ORDER BY symbol ASC")
        .map_err(|e| format!("vault list prepare failed: {e}"))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|e| format!("vault list query failed: {e}"))?;
    let mut out = Vec::new();
    for symbol in rows.flatten() {
        out.push(normalize_symbol(&symbol));
    }
    Ok(out)
}

pub fn has_symbol(symbol: &str) -> Result<bool, String> {
    let conn = open_db()?;
    let mut stmt = conn
        .prepare("SELECT 1 FROM secrets WHERE symbol = ?1 LIMIT 1")
        .map_err(|e| format!("vault has prepare failed: {e}"))?;
    let mut rows = stmt
        .query(params![normalize_symbol(symbol)])
        .map_err(|e| format!("vault has query failed: {e}"))?;
    match rows.next() {
        Ok(Some(_)) => Ok(true),
        Ok(None) => Ok(false),
        Err(e) => Err(format!("vault has row read failed: {e}")),
    }
}

pub fn load_legacy_kv_into_db_if_present() -> Result<(), String> {
    let legacy_path = env::var("HARMONIA_VAULT_STORE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| state_root().join("vault.secrets"));
    if !legacy_path.exists() {
        return Ok(());
    }
    let body = std::fs::read_to_string(&legacy_path)
        .map_err(|e| format!("vault legacy read failed: {e}"))?;
    for line in body.lines() {
        if let Some((k, v)) = line.split_once('=') {
            upsert_secret(k, v)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    const TEST_ENV_KEYS: &[&str] = &[
        "HOME",
        "HARMONIA_WALLET_ROOT",
        "HARMONIA_VAULT_WALLET_DB",
        "HARMONIA_WALLET_DB",
        "HARMONIIS_WALLET_DB",
        "HARMONIA_VAULT_DB",
        "HARMONIA_VAULT_MASTER_KEY",
        "HARMONIA_VAULT_ALLOW_UNENCRYPTED",
        "HARMONIA_VAULT_SLOT_FAMILY",
    ];

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
                    .map(|key| (*key, env::var_os(key)))
                    .collect::<Vec<_>>(),
            )
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in &self.0 {
                if let Some(saved) = value {
                    env::set_var(key, saved);
                } else {
                    env::remove_var(key);
                }
            }
        }
    }

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let path = env::temp_dir().join(format!("{prefix}-{}-{ts}", std::process::id()));
        std::fs::create_dir_all(&path).expect("temp dir");
        path
    }

    #[test]
    fn wallet_slot_root_encrypts_and_decrypts() {
        let _guard = acquire_env_lock();
        let _snapshot = EnvSnapshot::capture(TEST_ENV_KEYS);

        let root = make_temp_dir("harmonia-vault-wallet-test");
        let wallet_db = root.join("master.db");
        let vault_db = root.join("vault.db");

        let wallet_conn = Connection::open(&wallet_db).expect("open wallet");
        wallet_conn
            .execute(
                "CREATE TABLE wallet_slots (family TEXT NOT NULL, slot_index INTEGER NOT NULL, descriptor TEXT NOT NULL)",
                [],
            )
            .expect("wallet schema");
        wallet_conn
            .execute(
                "INSERT INTO wallet_slots(family, slot_index, descriptor) VALUES ('vault', 0, ?1)",
                params!["dfbb7b8a4fc6e869a3449a580493d7b8df82926d049e9e9eaff345b274e6b368"],
            )
            .expect("wallet slot insert");

        env::set_var("HOME", &root);
        env::set_var("HARMONIA_VAULT_WALLET_DB", &wallet_db);
        env::set_var("HARMONIA_VAULT_DB", &vault_db);
        env::remove_var("HARMONIA_VAULT_MASTER_KEY");
        env::remove_var("HARMONIA_VAULT_ALLOW_UNENCRYPTED");

        upsert_secret("openrouter", "sk-test-123").expect("upsert secret");

        let vault_conn = Connection::open(&vault_db).expect("open vault");
        let stored: String = vault_conn
            .query_row(
                "SELECT value FROM secrets WHERE symbol='openrouter'",
                [],
                |row| row.get(0),
            )
            .expect("read stored");
        assert!(stored.starts_with(AEAD_PREFIX));

        let loaded = load_store_file();
        assert_eq!(
            loaded.get("openrouter").map(String::as_str),
            Some("sk-test-123")
        );

        env::remove_var("HARMONIA_VAULT_WALLET_DB");
        env::remove_var("HARMONIA_VAULT_DB");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn write_fails_without_any_key_root() {
        let _guard = acquire_env_lock();
        let _snapshot = EnvSnapshot::capture(TEST_ENV_KEYS);
        let root = make_temp_dir("harmonia-vault-nokey-test");
        let vault_db = root.join("vault.db");

        env::set_var("HOME", &root);
        env::remove_var("HARMONIA_VAULT_WALLET_DB");
        env::remove_var("HARMONIA_WALLET_DB");
        env::remove_var("HARMONIIS_WALLET_DB");
        env::set_var("HARMONIA_VAULT_DB", &vault_db);
        env::remove_var("HARMONIA_VAULT_MASTER_KEY");
        env::remove_var("HARMONIA_VAULT_ALLOW_UNENCRYPTED");

        let err = upsert_secret("x", "y").expect_err("should fail");
        assert!(err.contains("vault encryption key unavailable"));

        env::remove_var("HARMONIA_VAULT_DB");
        let _ = std::fs::remove_dir_all(&root);
    }
}
