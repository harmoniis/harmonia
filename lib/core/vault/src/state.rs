use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

/// The in-memory secrets map type. Public so VaultActor can own one directly.
pub type SecretsMap = HashMap<String, String>;

static SECRETS: OnceLock<RwLock<SecretsMap>> = OnceLock::new();

pub fn secrets() -> &'static RwLock<SecretsMap> {
    SECRETS.get_or_init(|| RwLock::new(HashMap::new()))
}
