//! DynamicRegistry — pluggable component routing without hardcoded slots.
//!
//! Pure functional state transitions: every mutation clones the immutable
//! snapshot, applies changes, and swaps atomically. Wait-free reads via ArcSwap.

use std::collections::HashMap;
use std::sync::Arc;
use arc_swap::ArcSwap;
use ractor::ActorRef;
use crate::actors::ComponentMsg;

#[derive(Clone, Default)]
struct RegistryState {
    components: HashMap<String, ActorRef<ComponentMsg>>,
    capabilities: HashMap<String, Vec<String>>,
    topics: HashMap<String, Vec<String>>,
}

/// Dynamic component registry. Primary IPC dispatch path.
pub struct DynamicRegistry {
    state: ArcSwap<RegistryState>,
}

impl DynamicRegistry {
    pub fn new() -> Self {
        Self { state: ArcSwap::from_pointee(RegistryState::default()) }
    }

    /// Register a component with capabilities. Immutable swap.
    pub fn register(&self, name: &str, actor: ActorRef<ComponentMsg>, capabilities: &[&str]) {
        let name_s = name.to_string();
        let caps: Vec<String> = capabilities.iter().map(|c| c.to_string()).collect();
        let next = {
            let mut s = (*self.state.load_full()).clone();
            s.components.insert(name_s.clone(), actor);
            caps.iter().for_each(|cap|
                s.topics.entry(cap.clone()).or_default().push(name_s.clone()));
            s.capabilities.insert(name_s, caps);
            s
        };
        self.state.store(Arc::new(next));
    }

    /// Look up by name. Wait-free: atomic load + HashMap get.
    pub fn get(&self, name: &str) -> Option<ActorRef<ComponentMsg>> {
        self.state.load().components.get(name).cloned()
    }

    /// All registered component names.
    pub fn components(&self) -> Vec<String> {
        self.state.load().components.keys().cloned().collect()
    }

    /// Components subscribed to a capability topic.
    pub fn subscribers(&self, topic: &str) -> Vec<String> {
        self.state.load().topics.get(topic).cloned().unwrap_or_default()
    }

    /// Capabilities declared by a component.
    pub fn capabilities_of(&self, name: &str) -> Vec<String> {
        self.state.load().capabilities.get(name).cloned().unwrap_or_default()
    }

    /// Unregister on crash/shutdown. Functional: filter topics, swap.
    pub fn unregister(&self, name: &str) {
        let next = {
            let mut s = (*self.state.load_full()).clone();
            s.components.remove(name);
            if let Some(caps) = s.capabilities.remove(name) {
                caps.iter().for_each(|cap| {
                    s.topics.entry(cap.clone()).and_modify(|v| v.retain(|n| n != name));
                });
            }
            s
        };
        self.state.store(Arc::new(next));
    }

    pub fn len(&self) -> usize {
        self.state.load().components.len()
    }
}

pub type SharedDynamicRegistry = Arc<DynamicRegistry>;

pub fn new_dynamic() -> SharedDynamicRegistry {
    Arc::new(DynamicRegistry::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_registry_is_empty() {
        let reg = DynamicRegistry::new();
        assert_eq!(reg.len(), 0);
        assert!(reg.components().is_empty());
        assert!(reg.get("nonexistent").is_none());
    }

    #[test]
    fn test_subscribers_empty_topic() {
        let reg = DynamicRegistry::new();
        assert!(reg.subscribers("nonexistent").is_empty());
    }
}
