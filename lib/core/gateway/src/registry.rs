use crate::frontend_ffi::FrontendVtable;
use crate::model::SecurityLabel;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};

/// A capability declared by a frontend in its baseband config.
/// Capabilities are key-value pairs parsed from `:capabilities` in the config sexp.
/// Example: `(:a2ui "1.0" :push t)` → `[("a2ui", "1.0"), ("push", "t")]`
#[derive(Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub value: String,
}

pub struct FrontendHandle {
    pub vtable: FrontendVtable,
    pub security_label: SecurityLabel,
    pub config_sexp: String,
    pub so_path: String,
    pub capabilities: Vec<Capability>,
    pub crash_count: AtomicU32,
}

impl FrontendHandle {
    /// Check if this frontend declares a specific capability.
    pub fn has_capability(&self, name: &str) -> bool {
        self.capabilities.iter().any(|c| c.name == name)
    }

    /// Get the value of a declared capability, if present.
    pub fn capability_value(&self, name: &str) -> Option<&str> {
        self.capabilities
            .iter()
            .find(|c| c.name == name)
            .map(|c| c.value.as_str())
    }

    /// Render capabilities as an s-expression string.
    /// Returns `nil` if no capabilities are declared.
    pub fn capabilities_sexp(&self) -> String {
        if self.capabilities.is_empty() {
            return "nil".to_string();
        }
        let pairs: Vec<String> = self
            .capabilities
            .iter()
            .map(|c| format!(":{}  \"{}\"", c.name, c.value))
            .collect();
        format!("({})", pairs.join(" "))
    }
}

/// Parse capabilities from a config s-expression.
/// Looks for `:capabilities (...)` and extracts key-value pairs.
fn parse_capabilities(config_sexp: &str) -> Vec<Capability> {
    let needle = ":capabilities";
    let idx = match config_sexp.find(needle) {
        Some(i) => i,
        None => return Vec::new(),
    };
    let after = &config_sexp[idx + needle.len()..];
    let after = after.trim_start();
    if !after.starts_with('(') {
        return Vec::new();
    }
    let close = match after.find(')') {
        Some(i) => i,
        None => return Vec::new(),
    };
    let inner = &after[1..close];
    let mut caps = Vec::new();
    let mut chars = inner.chars().peekable();
    while let Some(&ch) = chars.peek() {
        if ch == ':' {
            chars.next(); // skip ':'
            let name: String = chars.by_ref().take_while(|c| !c.is_whitespace()).collect();
            // skip whitespace
            while chars.peek().map_or(false, |c| c.is_whitespace()) {
                chars.next();
            }
            // parse value: quoted string or bare token
            let value = if chars.peek() == Some(&'"') {
                chars.next(); // skip opening quote
                let v: String = chars.by_ref().take_while(|&c| c != '"').collect();
                v
            } else {
                let v: String = chars
                    .by_ref()
                    .take_while(|c| !c.is_whitespace() && *c != ')')
                    .collect();
                v
            };
            if !name.is_empty() {
                caps.push(Capability { name, value });
            }
        } else {
            chars.next();
        }
    }
    caps
}

pub struct Registry {
    frontends: RwLock<HashMap<String, FrontendHandle>>,
}

impl Registry {
    pub fn new() -> Self {
        Self {
            frontends: RwLock::new(HashMap::new()),
        }
    }

    pub fn register(
        &self,
        name: &str,
        so_path: &str,
        config_sexp: &str,
        security_label: SecurityLabel,
    ) -> Result<(), String> {
        let mut vtable = unsafe { FrontendVtable::load(name, so_path)? };
        vtable.init(config_sexp)?;
        let capabilities = parse_capabilities(config_sexp);
        let handle = FrontendHandle {
            vtable,
            security_label,
            config_sexp: config_sexp.to_string(),
            so_path: so_path.to_string(),
            capabilities,
            crash_count: AtomicU32::new(0),
        };
        self.frontends.write().insert(name.to_string(), handle);
        Ok(())
    }

    pub fn unregister(&self, name: &str) -> Result<(), String> {
        let mut map = self.frontends.write();
        if let Some(handle) = map.remove(name) {
            let _ = handle.vtable.shutdown();
            Ok(())
        } else {
            Err(format!("frontend not registered: {name}"))
        }
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.frontends.read().contains_key(name)
    }

    pub fn list_names(&self) -> Vec<String> {
        self.frontends.read().keys().cloned().collect()
    }

    pub fn with_frontend<F, R>(&self, name: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&FrontendHandle) -> R,
    {
        let map = self.frontends.read();
        map.get(name)
            .map(f)
            .ok_or_else(|| format!("frontend not registered: {name}"))
    }

    pub fn for_each<F>(&self, mut f: F)
    where
        F: FnMut(&str, &FrontendHandle),
    {
        let map = self.frontends.read();
        for (name, handle) in map.iter() {
            f(name, handle);
        }
    }

    /// Get the capabilities s-expression for a frontend.
    pub fn frontend_capabilities_sexp(&self, name: &str) -> Option<String> {
        let map = self.frontends.read();
        map.get(name).map(|h| h.capabilities_sexp())
    }

    pub fn reload(&self, name: &str) -> Result<(), String> {
        let mut map = self.frontends.write();
        let old_handle = map
            .remove(name)
            .ok_or_else(|| format!("frontend not registered: {name}"))?;

        let _ = old_handle.vtable.shutdown();

        let so_path = old_handle.so_path.clone();
        let config_sexp = old_handle.config_sexp.clone();
        let security_label = old_handle.security_label.clone();
        let prev_crash_count = old_handle.crash_count.load(Ordering::Relaxed);

        let mut vtable = unsafe { FrontendVtable::load(name, &so_path)? };
        vtable.init(&config_sexp)?;

        let capabilities = parse_capabilities(&config_sexp);
        let handle = FrontendHandle {
            vtable,
            security_label,
            config_sexp,
            so_path,
            capabilities,
            crash_count: AtomicU32::new(prev_crash_count),
        };
        map.insert(name.to_string(), handle);
        Ok(())
    }

    pub fn crash_count(&self, name: &str) -> Result<u32, String> {
        let map = self.frontends.read();
        map.get(name)
            .map(|h| h.crash_count.load(Ordering::Relaxed))
            .ok_or_else(|| format!("frontend not registered: {name}"))
    }

    pub fn shutdown_all(&self) {
        let mut map = self.frontends.write();
        for (_, handle) in map.drain() {
            let _ = handle.vtable.shutdown();
        }
    }
}
