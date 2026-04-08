//! Vault persistent store: encrypted secret storage backed by SQLite.

mod db;
mod encryption;
mod query;

pub use db::store_path;
pub use query::{
    derive_scoped_secret_hex, has_symbol, list_symbols, load_legacy_kv_into_db_if_present,
    load_store_file, normalize_env_symbol, normalize_symbol, upsert_secret,
};
