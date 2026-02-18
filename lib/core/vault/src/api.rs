use std::collections::HashMap;

use crate::ingest::ingest_env;
use crate::state::secrets;
use crate::store::{
    has_symbol, list_symbols, load_legacy_kv_into_db_if_present, load_store_file, normalize_symbol,
    upsert_secret,
};

fn load_all_sources(map: &mut HashMap<String, String>) {
    map.clear();
    for (k, v) in load_store_file() {
        map.insert(k, v);
    }
    ingest_env(map);
}

pub fn init_from_env() -> Result<(), String> {
    load_legacy_kv_into_db_if_present()?;
    let mut map = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;
    load_all_sources(&mut map);
    Ok(())
}

pub fn get_secret_for_symbol(symbol: &str) -> Option<String> {
    let normalized = normalize_symbol(symbol);
    secrets()
        .read()
        .ok()
        .and_then(|map| map.get(&normalized).cloned())
}

pub fn set_secret_for_symbol(symbol: &str, value: &str) -> Result<(), String> {
    let key = normalize_symbol(symbol);
    let mut map = secrets()
        .write()
        .map_err(|_| "vault lock poisoned".to_string())?;
    map.insert(key, value.to_string());
    upsert_secret(symbol, value)
}

pub fn has_secret_for_symbol(symbol: &str) -> bool {
    if let Ok(map) = secrets().read() {
        return map.contains_key(&normalize_symbol(symbol));
    }
    has_symbol(symbol).unwrap_or(false)
}

pub fn list_secret_symbols() -> Vec<String> {
    if let Ok(map) = secrets().read() {
        let mut keys: Vec<String> = map.keys().cloned().collect();
        keys.sort();
        keys.dedup();
        return keys;
    }
    list_symbols().unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_symbol_lookup() {
        {
            let mut map = crate::state::secrets().write().unwrap();
            map.insert("openrouter".to_string(), "k".to_string());
        }
        assert_eq!(get_secret_for_symbol(":OpenRouter").as_deref(), Some("k"));
    }
}
