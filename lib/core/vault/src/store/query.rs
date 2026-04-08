//! Public query API: load, upsert, list, has, legacy import, symbol normalization.

use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

use rusqlite::params;

use super::db::{open_db, state_root};
use super::encryption::{
    allow_unencrypted_writes, decrypt_value, encrypt_aead, resolve_encryption_key,
};

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
    use super::encryption::{derive_32, SCOPED_DERIVE_SALT};

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
    use super::super::encryption::AEAD_PREFIX;
    use rusqlite::{params, Connection};
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

        let result = upsert_secret("x", "y");
        assert!(result.is_err(), "should fail without key, but got Ok");
        let err = result.unwrap_err();
        assert!(
            err.contains("key") || err.contains("encrypt") || err.contains("unavailable"),
            "unexpected error: {err}"
        );

        env::remove_var("HARMONIA_VAULT_DB");
        let _ = std::fs::remove_dir_all(&root);
    }
}
