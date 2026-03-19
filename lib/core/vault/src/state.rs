use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

static SECRETS: OnceLock<RwLock<HashMap<String, String>>> = OnceLock::new();

pub fn secrets() -> &'static RwLock<HashMap<String, String>> {
    SECRETS.get_or_init(|| RwLock::new(HashMap::new()))
}
