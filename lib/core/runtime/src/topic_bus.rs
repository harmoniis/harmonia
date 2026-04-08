//! TopicBus — pub/sub message routing by capability topic.
//!
//! Pure functional: all state transitions produce new immutable snapshots
//! via ArcSwap clone-and-swap. Publish uses iterator fold, not imperative loop.

use std::collections::HashMap;
use std::sync::Arc;
use arc_swap::ArcSwap;
use ractor::ActorRef;
use crate::actors::ComponentMsg;

#[derive(Clone, Default)]
struct BusState {
    subscriptions: HashMap<String, Vec<ActorRef<ComponentMsg>>>,
}

pub struct TopicBus {
    state: ArcSwap<BusState>,
}

impl TopicBus {
    pub fn new() -> Self {
        Self { state: ArcSwap::from_pointee(BusState::default()) }
    }

    /// Subscribe an actor to a topic. Immutable swap: clone → modify → store.
    pub fn subscribe(&self, topic: &str, actor: ActorRef<ComponentMsg>) {
        let next = {
            let mut s = (*self.state.load_full()).clone();
            s.subscriptions.entry(topic.to_string()).or_default().push(actor);
            s
        };
        self.state.store(Arc::new(next));
    }

    /// Unsubscribe an actor from all topics. Functional filter via retain.
    pub fn unsubscribe_all(&self, actor: &ActorRef<ComponentMsg>) {
        let actor_id = actor.get_id();
        let next = {
            let mut s = (*self.state.load_full()).clone();
            s.subscriptions.values_mut()
                .for_each(|subs| subs.retain(|a| a.get_id() != actor_id));
            s
        };
        self.state.store(Arc::new(next));
    }

    /// Publish a sexp message to all subscribers. Returns delivery count.
    /// Functional: fold over subscribers, fire-and-forget each via cast.
    pub fn publish(&self, topic: &str, payload_sexp: &str) -> usize {
        let state = self.state.load();
        state.subscriptions.get(topic).map_or(0, |subs| {
            let msg = format!(
                "(:component \"topic-bus\" :op \"topic-message\" :topic \"{}\" :payload {})",
                topic, payload_sexp);
            subs.iter().filter(|actor| {
                let (tx, _rx) = ractor::concurrency::oneshot();
                actor.cast(ComponentMsg::Dispatch(msg.clone(), tx.into())).is_ok()
            }).count()
        })
    }

    /// List all topics with subscriber counts. Pure: snapshot → map → collect.
    pub fn topics(&self) -> Vec<(String, usize)> {
        self.state.load().subscriptions.iter()
            .map(|(t, s)| (t.clone(), s.len())).collect()
    }
}

pub type SharedTopicBus = Arc<TopicBus>;

pub fn new_topic_bus() -> SharedTopicBus {
    Arc::new(TopicBus::new())
}
