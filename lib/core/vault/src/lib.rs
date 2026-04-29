mod api;
mod ingest;
mod state;
mod store;

pub use api::{
    derive_component_seed_hex, get_secret_for_component, has_secret_for_symbol, init_from_env,
    list_secret_symbols, set_secret_for_symbol, ComponentPolicyMap,
    // Actor-owned API (preferred)
    init_state, get_secret_with_state, set_secret_with_state,
    has_secret_with_state, list_secrets_with_state,
};
pub use state::{SecretsMap, VaultState};
pub use store::store_path;
