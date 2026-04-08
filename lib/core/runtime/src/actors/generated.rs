//! Macro-generated actors.
//!
//! Stateful: declare_component! from ComponentDescriptor trait.
//! Stateless: declare_stateless_component! for components routed through dispatch::dispatch().

use crate::components;

// ComponentDescriptor-based actors: ONE line each.
crate::macros::declare_component!(components::MemPalaceComponent, MemPalaceActor, MemPalaceActorState);
crate::macros::declare_component!(components::TerraphonComponent, TerraphonActor, TerraphonActorState);
crate::macros::declare_component!(components::ChronicleComponent, ChronicleComponentActor, ChronicleComponentState);
crate::macros::declare_component!(components::OuroborosComponent, OuroborosActor, OuroborosActorState);
crate::macros::declare_component!(components::SessionComponent, SessionActor, SessionActorState);

// Stateless actors dispatching through the hardcoded match.
crate::macros::declare_stateless_component!(ConfigActor, "config");
crate::macros::declare_stateless_component!(WorkspaceActor, "workspace", blocking);
crate::macros::declare_stateless_component!(ProviderRouterActor, "provider-router", blocking);
crate::macros::declare_stateless_component!(ParallelActor, "parallel", blocking);
