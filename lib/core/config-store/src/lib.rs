mod store;

pub mod api;
mod ingest;
mod policy;
mod registry;

// v1 re-exports (simple scope/key API without policy enforcement)
pub use store::{get_value, init, list_keys, set_value};

// Public types for actor ownership (singleton elimination)
pub use store::{ConfigCache, ConfigDbConn};

// v2 public API (policy-gated, component-aware)
pub use api::{
    delete_config, dump_scope, get_config, get_config_or, get_own, get_own_or, init as init_v2,
    list_scope, set_config,
};
