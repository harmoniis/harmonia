use std::collections::HashMap;

/// The in-memory secrets map type.
pub type SecretsMap = HashMap<String, String>;

/// Actor-owned vault state. No global singletons.
pub struct VaultState {
    pub secrets: SecretsMap,
}

impl VaultState {
    pub fn new() -> Self {
        Self {
            secrets: HashMap::new(),
        }
    }
}

impl Default for VaultState {
    fn default() -> Self {
        Self::new()
    }
}
