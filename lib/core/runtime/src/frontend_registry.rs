//! Live registry of frontend actors.
//!
//! Each frontend that has been migrated to the trait-based actor model
//! ([`harmonia_frontend_trait::Frontend`]) is spawned at runtime boot and
//! registered here. The registry stores both the [`ActorRef<FrontendMsg>`]
//! for poll/send calls and the frontend's static security label, so the
//! gateway dispatch can build correctly-labelled envelopes without holding
//! a typed reference to each frontend.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use harmonia_frontend_trait::FrontendMsg;
use ractor::ActorRef;

#[derive(Clone)]
pub struct FrontendEntry {
    pub actor: ActorRef<FrontendMsg>,
    pub security_label: &'static str,
}

#[derive(Clone, Default)]
pub struct FrontendRegistry {
    inner: Arc<RwLock<HashMap<String, FrontendEntry>>>,
}

impl FrontendRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&self, name: &str, entry: FrontendEntry) {
        if let Ok(mut g) = self.inner.write() {
            g.insert(name.to_string(), entry);
        }
    }

    pub fn get(&self, name: &str) -> Option<FrontendEntry> {
        self.inner.read().ok().and_then(|g| g.get(name).cloned())
    }

    pub fn entries(&self) -> Vec<(String, FrontendEntry)> {
        self.inner
            .read()
            .ok()
            .map(|g| g.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
            .unwrap_or_default()
    }

    pub fn contains(&self, name: &str) -> bool {
        self.inner
            .read()
            .ok()
            .map(|g| g.contains_key(name))
            .unwrap_or(false)
    }
}
