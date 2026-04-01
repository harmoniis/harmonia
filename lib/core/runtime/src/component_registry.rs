//! Component Actor Registry — lock-free direct dispatch.
//!
//! The IPC handler dispatches component calls DIRECTLY to actor mailboxes
//! via this shared registry. The supervisor is never in the hot path.
//!
//!   IPC → component actor → dispatch → result
//!
//! Uses a slot-indexed array: lookup is a single array index,
//! no hashing, no locking, no allocation on the read path.
//! Writes (actor restart) use ArcSwap for wait-free reads.

use std::sync::Arc;

use arc_swap::ArcSwap;
use ractor::ActorRef;

use crate::actors::ComponentMsg;

/// Fixed component slots — indexed by discriminant for O(1) lookup.
#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum ComponentSlot {
    Chronicle = 0,
    Gateway = 1,
    Tailnet = 2,
    Signalograd = 3,
    MemoryField = 4,
    Vault = 5,
    Config = 6,
    ProviderRouter = 7,
    Parallel = 8,
    Router = 9,
    GitOps = 10,
    Ouroboros = 11,
    Workspace = 12,
}

pub const NUM_SLOTS: usize = 13;

impl ComponentSlot {
    /// Map a component name to its slot. Returns None for unknown names.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "chronicle" => Some(Self::Chronicle),
            "gateway" => Some(Self::Gateway),
            "tailnet" => Some(Self::Tailnet),
            "signalograd" => Some(Self::Signalograd),
            "memory-field" => Some(Self::MemoryField),
            "vault" => Some(Self::Vault),
            "config" => Some(Self::Config),
            "provider-router" => Some(Self::ProviderRouter),
            "parallel" => Some(Self::Parallel),
            "router" => Some(Self::Router),
            "git-ops" => Some(Self::GitOps),
            "ouroboros" => Some(Self::Ouroboros),
            "workspace" => Some(Self::Workspace),
            _ => None,
        }
    }
}

/// Inner array — immutable once constructed, swapped atomically on restart.
#[derive(Clone)]
pub struct RegistryInner {
    slots: [Option<ActorRef<ComponentMsg>>; NUM_SLOTS],
}

impl RegistryInner {
    fn empty() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }
}

/// Shared component actor registry.
/// ArcSwap gives wait-free reads (a single atomic load on the hot path).
/// Writes clone-and-swap, which is fine for the rare restart case.
pub type SharedRegistry = Arc<ArcSwap<RegistryInner>>;

/// Create an empty registry.
pub fn new() -> SharedRegistry {
    Arc::new(ArcSwap::from_pointee(RegistryInner::empty()))
}

/// Register a component actor by slot name.
/// Clones the inner array and swaps atomically — O(1) read path unaffected.
pub fn register(registry: &SharedRegistry, name: &str, actor: ActorRef<ComponentMsg>) {
    if let Some(slot) = ComponentSlot::from_name(name) {
        let mut inner = (**registry.load()).clone();
        inner.slots[slot as usize] = Some(actor);
        registry.store(Arc::new(inner));
    }
}

/// Look up a component actor by name. Wait-free: one atomic load + array index.
pub fn get(registry: &SharedRegistry, name: &str) -> Option<ActorRef<ComponentMsg>> {
    let slot = ComponentSlot::from_name(name)?;
    let inner = registry.load();
    inner.slots[slot as usize].clone()
}
