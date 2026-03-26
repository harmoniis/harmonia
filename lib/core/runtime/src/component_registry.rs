//! Component Actor Registry — direct dispatch, no supervisor bottleneck.
//!
//! The IPC handler dispatches component calls DIRECTLY to actor mailboxes
//! via this shared registry. The supervisor is never in the hot path.
//!
//! This eliminates the supervisor single-threaded bottleneck:
//!   Before: IPC → supervisor → component actor → dispatch → result
//!   After:  IPC → component actor → dispatch → result
//!
//! The supervisor only handles lifecycle (register, restart, module management).
//! Data calls bypass it entirely.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use ractor::ActorRef;

use crate::actors::ComponentMsg;

/// Shared component actor registry.
/// Arc<RwLock<>> for concurrent reads from IPC tasks, rare writes on actor registration.
pub type SharedRegistry = Arc<RwLock<HashMap<String, ActorRef<ComponentMsg>>>>;

/// Create an empty registry.
pub fn new() -> SharedRegistry {
    Arc::new(RwLock::new(HashMap::new()))
}

/// Register a component actor. Called from main.rs during startup.
pub fn register(registry: &SharedRegistry, name: &str, actor: ActorRef<ComponentMsg>) {
    if let Ok(mut map) = registry.write() {
        map.insert(name.to_string(), actor);
    }
}

/// Look up a component actor by name. Lock-free read path.
pub fn get(registry: &SharedRegistry, name: &str) -> Option<ActorRef<ComponentMsg>> {
    registry.read().ok().and_then(|map| map.get(name).cloned())
}
