mod sqlite;

use crate::model::{State, StoreKind};

use super::shared::{reset_state, state, store_config};

#[allow(dead_code)]
pub(super) fn persist_if_needed(st: &State) -> Result<(), String> {
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();
    match cfg.kind {
        StoreKind::Memory => Ok(()),
        StoreKind::Sqlite => sqlite::persist_state_sqlite(st, &cfg),
        StoreKind::Graph => Err("graph store adapter contract exists but implementation is pending; use memory|sqlite for now".to_string()),
    }
}

#[allow(dead_code)]
pub(crate) fn set_store(kind: &str, path_override: Option<&str>) -> Result<(), String> {
    let parsed = if kind.eq_ignore_ascii_case("memory") {
        StoreKind::Memory
    } else if kind.eq_ignore_ascii_case("sqlite") || kind.eq_ignore_ascii_case("sql") {
        StoreKind::Sqlite
    } else if kind.eq_ignore_ascii_case("graph") {
        StoreKind::Graph
    } else {
        return Err("invalid store kind, expected memory|sqlite|graph".to_string());
    };

    if parsed == StoreKind::Graph {
        return Err(
            "graph store adapter contract exists but implementation is pending; use memory|sqlite for now"
                .to_string(),
        );
    }

    let mut cfg = store_config()
        .write()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?;
    cfg.kind = parsed;
    if let Some(path) = path_override {
        if !path.trim().is_empty() {
            cfg.path = path.to_string();
        }
    }
    let cfg_now = cfg.clone();
    drop(cfg);

    if cfg_now.kind == StoreKind::Sqlite {
        let mut st = state()
            .write()
            .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;
        if let Some(loaded) = sqlite::load_state_sqlite(&cfg_now)? {
            *st = loaded;
        } else {
            sqlite::persist_state_sqlite(&st, &cfg_now)?;
        }
    }

    Ok(())
}

#[allow(dead_code)]
pub(crate) fn init() -> Result<(), String> {
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();

    let mut st = state()
        .write()
        .map_err(|_| "harmonic matrix state lock poisoned".to_string())?;

    if cfg.kind == StoreKind::Sqlite {
        if let Some(loaded) = sqlite::load_state_sqlite(&cfg)? {
            *st = loaded;
            return Ok(());
        }
    } else if cfg.kind == StoreKind::Graph {
        return Err("graph store adapter contract exists but implementation is pending; use memory|sqlite for now".to_string());
    }

    reset_state(&mut st);
    persist_if_needed(&st)?;
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn store_summary() -> Result<String, String> {
    let cfg = store_config()
        .read()
        .map_err(|_| "harmonic matrix store config lock poisoned".to_string())?
        .clone();
    Ok(format!(
        "(:store-kind \"{}\" :store-path \"{}\")",
        cfg.kind_name(),
        cfg.path.replace('"', "\\\"")
    ))
}
