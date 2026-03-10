mod ffi;
mod state;
mod store;

pub mod api;
mod ingest;
mod legacy;
mod policy;

// Legacy re-exports (backward compat)
pub use store::{get_value, init, list_keys, set_value};

// v2 public API
pub use api::{
    delete_config, dump_scope, get_config, get_config_or, get_own, get_own_or, init as init_v2,
    list_scope, set_config,
};
