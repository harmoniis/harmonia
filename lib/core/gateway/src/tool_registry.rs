use crate::model::SecurityLabel;
use harmonia_baseband_channel_protocol::ChannelBody;
use harmonia_tool_channel_protocol::{next_request_id, ToolResult, ToolStatus};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64};

pub struct ToolHandle {
    pub security_label: SecurityLabel,
    pub config_sexp: String,
    pub name: String,
    pub actor_id: AtomicU64,
    pub crash_count: AtomicU32,
}

pub struct ToolRegistry {
    tools: RwLock<HashMap<String, ToolHandle>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool by name.
    ///
    /// Dynamic library loading has been removed -- tools are now compiled
    /// into harmonia-runtime as ractor actors. This method registers metadata
    /// only.
    pub fn register(
        &self,
        name: &str,
        config_sexp: &str,
        security_label: SecurityLabel,
    ) -> Result<(), String> {
        let handle = ToolHandle {
            security_label,
            config_sexp: config_sexp.to_string(),
            name: name.to_string(),
            actor_id: AtomicU64::new(0),
            crash_count: AtomicU32::new(0),
        };
        self.tools.write().insert(name.to_string(), handle);
        Ok(())
    }

    pub fn unregister(&self, name: &str) -> Result<(), String> {
        let mut map = self.tools.write();
        if map.remove(name).is_some() {
            Ok(())
        } else {
            Err(format!("tool not registered: {name}"))
        }
    }

    /// Invoke is now a stub -- tool invocations are dispatched via the runtime
    /// actor system. This remains for API compatibility but always returns an
    /// error directing callers to the actor-based path.
    pub fn invoke(&self, name: &str, operation: &str, _params: &str) -> Result<ToolResult, String> {
        let map = self.tools.read();
        let _handle = map
            .get(name)
            .ok_or_else(|| format!("tool not registered: {name}"))?;

        let request_id = next_request_id();
        Ok(ToolResult {
            request_id,
            tool_name: name.to_string(),
            operation: operation.to_string(),
            status: ToolStatus::Error(
                "FFI tool invocation removed; use actor-based dispatch".to_string(),
            ),
            body: ChannelBody::text(
                "FFI tool invocation removed; use actor-based dispatch".to_string(),
            ),
            duration_ms: 0,
            dissonance: 0.0,
        })
    }

    pub fn is_registered(&self, name: &str) -> bool {
        self.tools.read().contains_key(name)
    }

    pub fn list_names(&self) -> Vec<String> {
        self.tools.read().keys().cloned().collect()
    }

    pub fn with_tool<F, R>(&self, name: &str, f: F) -> Result<R, String>
    where
        F: FnOnce(&ToolHandle) -> R,
    {
        let map = self.tools.read();
        map.get(name)
            .map(f)
            .ok_or_else(|| format!("tool not registered: {name}"))
    }

    pub fn capabilities(&self, name: &str) -> Result<String, String> {
        let map = self.tools.read();
        let _handle = map
            .get(name)
            .ok_or_else(|| format!("tool not registered: {name}"))?;
        Ok("nil".to_string())
    }

    pub fn shutdown_all(&self) {
        let mut map = self.tools.write();
        map.drain();
    }
}
