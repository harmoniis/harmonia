use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// The in-memory secrets map type. Public so VaultActor can own one directly.
pub type SecretsMap = HashMap<String, String>;

/// Legacy global secrets store — deprecated. Use actor-owned SecretsMap instead.
static LEGACY_SECRETS: OnceLock<RwLock<SecretsMap>> = OnceLock::new();

/// Legacy accessor — returns the global singleton. Deprecated.
pub fn secrets() -> &'static RwLock<SecretsMap> {
    LEGACY_SECRETS.get_or_init(|| RwLock::new(HashMap::new()))
}
