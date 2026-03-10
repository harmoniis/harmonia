/// Policy-gated public API with env fallback chain.
use std::env;

use crate::legacy::{canonical_env_name, legacy_env_name};
use crate::policy;
use crate::store;

/// Initialize the config store: open DB, load cache, seed from env on first run.
pub fn init() -> Result<(), String> {
    store::init()?;
    crate::ingest::seed_from_env()?;
    Ok(())
}

/// Get a config value with full fallback chain:
/// 1. In-memory cache / DB
/// 2. Legacy env var alias
/// 3. Canonical env var `HARMONIA_{SCOPE}_{KEY}`
/// 4. None
pub fn get_config(component: &str, scope: &str, key: &str) -> Result<Option<String>, String> {
    let scope = store::norm_scope(scope);
    let key = store::norm_key(key);
    if !policy::can_read(component, &scope) {
        return Err(format!(
            "config-store policy denied read: component='{}' scope='{}'",
            component, scope
        ));
    }

    // 1. Cache / DB lookup
    if let Some(v) = store::cache_get(&scope, &key) {
        return Ok(Some(v));
    }
    if let Ok(Some(v)) = store::get_value(&scope, &key) {
        return Ok(Some(v));
    }

    // 2. Legacy env alias
    if let Some(env_name) = legacy_env_name(&scope, &key) {
        if let Ok(v) = env::var(env_name) {
            let v = v.trim().to_string();
            if !v.is_empty() {
                return Ok(Some(v));
            }
        }
    }

    // 3. Canonical env var
    let canonical = canonical_env_name(&scope, &key);
    if let Ok(v) = env::var(&canonical) {
        let v = v.trim().to_string();
        if !v.is_empty() {
            return Ok(Some(v));
        }
    }

    Ok(None)
}

/// Get with a default fallback.
pub fn get_config_or(
    component: &str,
    scope: &str,
    key: &str,
    default: &str,
) -> Result<String, String> {
    Ok(get_config(component, scope, key)?.unwrap_or_else(|| default.to_string()))
}

/// Convenience: get from the component's own scope.
pub fn get_own(component: &str, key: &str) -> Result<Option<String>, String> {
    get_config(component, component, key)
}

/// Convenience: get from own scope with default.
pub fn get_own_or(component: &str, key: &str, default: &str) -> Result<String, String> {
    get_config_or(component, component, key, default)
}

/// Set a config value (policy-gated).
pub fn set_config(component: &str, scope: &str, key: &str, value: &str) -> Result<(), String> {
    let scope = store::norm_scope(scope);
    if !policy::can_write(component, &scope) {
        return Err(format!(
            "config-store policy denied write: component='{}' scope='{}'",
            component, scope
        ));
    }
    store::set_value(&scope, key, value)
}

/// Delete a config value (admin-only).
pub fn delete_config(component: &str, scope: &str, key: &str) -> Result<(), String> {
    let scope = store::norm_scope(scope);
    if !policy::can_delete(component, &scope) {
        return Err(format!(
            "config-store policy denied delete: component='{}' scope='{}'",
            component, scope
        ));
    }
    store::delete_value(&scope, key)
}

/// List all keys in a scope (policy-gated read).
pub fn list_scope(component: &str, scope: &str) -> Result<Vec<String>, String> {
    let scope = store::norm_scope(scope);
    if !policy::can_read(component, &scope) {
        return Err(format!(
            "config-store policy denied list: component='{}' scope='{}'",
            component, scope
        ));
    }
    store::list_keys(Some(&scope))
}

/// Dump all (key, value) pairs in a scope (policy-gated read).
pub fn dump_scope(component: &str, scope: &str) -> Result<Vec<(String, String)>, String> {
    let scope = store::norm_scope(scope);
    if !policy::can_read(component, &scope) {
        return Err(format!(
            "config-store policy denied dump: component='{}' scope='{}'",
            component, scope
        ));
    }
    store::dump_scope(&scope)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admin_can_read_any_scope() {
        // This tests the policy integration — admin components bypass scope checks.
        let result = get_config("conductor", "openai-backend", "base-url");
        assert!(result.is_ok());
    }

    #[test]
    fn component_denied_cross_scope() {
        let result = get_config("openai-backend", "anthropic-backend", "api-version");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("policy denied"));
    }

    #[test]
    fn component_reads_own_scope() {
        let result = get_config("openai-backend", "openai-backend", "base-url");
        assert!(result.is_ok());
    }

    #[test]
    fn get_own_uses_component_as_scope() {
        let result = get_own("openai-backend", "base-url");
        assert!(result.is_ok());
    }

    #[test]
    fn write_own_scope_ok() {
        let result = set_config("openai-backend", "openai-backend", "test-key", "val");
        // May fail if DB not initialized, but should not fail on policy
        if let Err(e) = &result {
            assert!(!e.contains("policy denied"));
        }
    }

    #[test]
    fn write_other_scope_denied() {
        let result = set_config("openai-backend", "anthropic-backend", "key", "val");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("policy denied"));
    }

    #[test]
    fn delete_non_admin_denied() {
        let result = delete_config("openai-backend", "openai-backend", "key");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("policy denied"));
    }
}
