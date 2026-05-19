//! Canonical Harmonia path resolution. Single source of truth.
//!
//! Resolution order for `state-root` (the directory that holds vault.db,
//! chronicle.db, config.db, metrics.db, sockets, caches):
//!
//!   1. `HARMONIA_STATE_ROOT` env var (zero-dependency fast path; Phoenix
//!      sets this for every supervised child, so it works before the
//!      config-store DB is opened and survives any policy/cache race).
//!   2. `config-store` scope=`global` key=`state-root` (runtime override
//!      written by `harmonia setup`; only tried if the env is unset).
//!   3. `env::temp_dir().join("harmonia")` (last-resort default — only
//!      reached in tests or stand-alone tools).
//!
//! Every component that needs state-root or one of the canonical
//! databases MUST go through this module. Rolling your own causes the
//! exact divergence we used to have, where some callers honored env and
//! others only honored the DB, producing split-brain processes opening
//! different chronicle / vault files inside the same daemon.

use std::env;
use std::path::PathBuf;

/// Resolve the Harmonia state directory. See module docs for the
/// resolution chain.
pub fn state_root() -> PathBuf {
    if let Some(p) = env_path("HARMONIA_STATE_ROOT") {
        return p;
    }
    if let Ok(Some(v)) = crate::api::get_config("config-store", "global", "state-root") {
        if let Some(p) = trim_to_path(&v) {
            return p;
        }
    }
    env::temp_dir().join("harmonia")
}

/// Resolve a state-root-relative child path, honoring a per-target env
/// override first. Use this when a single canonical filename inside
/// state-root represents the asset (e.g. `chronicle.db`).
pub fn state_child(env_var: &str, file: &str) -> PathBuf {
    env_path(env_var).unwrap_or_else(|| state_root().join(file))
}

/// Canonical DB paths. Every component that touches one of these
/// databases must use the corresponding helper here, never compute the
/// path itself.
pub fn chronicle_db() -> PathBuf {
    state_child("HARMONIA_CHRONICLE_DB", "chronicle.db")
}
pub fn config_db() -> PathBuf {
    state_child("HARMONIA_CONFIG_DB", "config.db")
}
pub fn vault_db() -> PathBuf {
    state_child("HARMONIA_VAULT_DB", "vault.db")
}
pub fn metrics_db() -> PathBuf {
    state_child("HARMONIA_METRICS_DB", "metrics.db")
}

fn env_path(var: &str) -> Option<PathBuf> {
    env::var(var).ok().and_then(|v| trim_to_path(&v))
}

fn trim_to_path(s: &str) -> Option<PathBuf> {
    let t = s.trim();
    if t.is_empty() {
        None
    } else {
        Some(PathBuf::from(t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `HARMONIA_STATE_ROOT` short-circuits ahead of the config-store DB
    /// lookup — the entire point of the env-first chain.
    #[test]
    fn env_wins_over_default() {
        let key = "HARMONIA_STATE_ROOT";
        let prev = env::var(key).ok();
        env::set_var(key, "/tmp/harmonia-paths-test-env");
        assert_eq!(state_root(), PathBuf::from("/tmp/harmonia-paths-test-env"));
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }

    /// Empty / whitespace-only env values fall through, not silently
    /// returning `""` and breaking every downstream `.join(...)`.
    #[test]
    fn blank_env_falls_through() {
        let key = "HARMONIA_STATE_ROOT";
        let prev = env::var(key).ok();
        env::set_var(key, "   ");
        let r = state_root();
        assert_ne!(r, PathBuf::from(""));
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }

    /// Per-asset env overrides win over the derived `state_root/file` path.
    #[test]
    fn asset_env_override_wins() {
        let key = "HARMONIA_CHRONICLE_DB";
        let prev = env::var(key).ok();
        env::set_var(key, "/tmp/harmonia-paths-test/explicit-chronicle.db");
        assert_eq!(
            chronicle_db(),
            PathBuf::from("/tmp/harmonia-paths-test/explicit-chronicle.db")
        );
        match prev {
            Some(v) => env::set_var(key, v),
            None => env::remove_var(key),
        }
    }
}
