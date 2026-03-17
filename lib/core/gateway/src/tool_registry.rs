use crate::model::SecurityLabel;
use crate::tool_ffi::ToolVtable;
use harmonia_baseband_channel_protocol::ChannelBody;
use harmonia_signal_integrity::{compute_dissonance, scan_for_injection};
use harmonia_tool_channel_protocol::{next_request_id, ToolResult, ToolStatus};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Instant;

pub struct ToolHandle {
    pub vtable: ToolVtable,
    pub security_label: SecurityLabel,
    pub config_sexp: String,
    pub so_path: String,
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

    pub fn register(
        &self,
        name: &str,
        so_path: &str,
        config_sexp: &str,
        security_label: SecurityLabel,
    ) -> Result<(), String> {
        let mut vtable = unsafe { ToolVtable::load(name, so_path)? };
        vtable.init(config_sexp)?;
        let handle = ToolHandle {
            vtable,
            security_label,
            config_sexp: config_sexp.to_string(),
            so_path: so_path.to_string(),
            actor_id: AtomicU64::new(0),
            crash_count: AtomicU32::new(0),
        };
        self.tools.write().insert(name.to_string(), handle);
        Ok(())
    }

    pub fn unregister(&self, name: &str) -> Result<(), String> {
        let mut map = self.tools.write();
        if let Some(handle) = map.remove(name) {
            let _ = handle.vtable.shutdown();
            Ok(())
        } else {
            Err(format!("tool not registered: {name}"))
        }
    }

    pub fn invoke(&self, name: &str, operation: &str, params: &str) -> Result<ToolResult, String> {
        let map = self.tools.read();
        let handle = map
            .get(name)
            .ok_or_else(|| format!("tool not registered: {name}"))?;

        let request_id = next_request_id();
        let start = Instant::now();

        match handle.vtable.invoke(operation, params) {
            Ok(raw_output) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                let report = scan_for_injection(&raw_output);
                let dissonance = compute_dissonance(&report);

                Ok(ToolResult {
                    request_id,
                    tool_name: name.to_string(),
                    operation: operation.to_string(),
                    status: ToolStatus::Success,
                    body: ChannelBody::text(raw_output),
                    duration_ms,
                    dissonance,
                })
            }
            Err(e) => {
                let duration_ms = start.elapsed().as_millis() as u64;
                Ok(ToolResult {
                    request_id,
                    tool_name: name.to_string(),
                    operation: operation.to_string(),
                    status: ToolStatus::Error(e.clone()),
                    body: ChannelBody::text(e),
                    duration_ms,
                    dissonance: 0.0,
                })
            }
        }
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
        let handle = map
            .get(name)
            .ok_or_else(|| format!("tool not registered: {name}"))?;
        Ok(handle.vtable.capabilities())
    }

    pub fn reload(&self, name: &str) -> Result<(), String> {
        let mut map = self.tools.write();
        let old_handle = map
            .remove(name)
            .ok_or_else(|| format!("tool not registered: {name}"))?;

        let _ = old_handle.vtable.shutdown();

        let so_path = old_handle.so_path.clone();
        let config_sexp = old_handle.config_sexp.clone();
        let security_label = old_handle.security_label;
        let prev_crash_count = old_handle.crash_count.load(Ordering::Relaxed);

        let mut vtable = unsafe { ToolVtable::load(name, &so_path)? };
        vtable.init(&config_sexp)?;

        let handle = ToolHandle {
            vtable,
            security_label,
            config_sexp,
            so_path,
            actor_id: AtomicU64::new(0),
            crash_count: AtomicU32::new(prev_crash_count),
        };
        map.insert(name.to_string(), handle);
        Ok(())
    }

    pub fn shutdown_all(&self) {
        let mut map = self.tools.write();
        for (_, handle) in map.drain() {
            let _ = handle.vtable.shutdown();
        }
    }
}
