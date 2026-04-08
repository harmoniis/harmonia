//! Component definitions — descriptors for stateful components, capability
//! declarations for all components (used by DynamicRegistry for topic routing).

use harmonia_actor_protocol::ComponentDescriptor;

// ── Stateful Components (own state, dispatch via ComponentDescriptor) ────

pub struct MemPalaceComponent;
impl ComponentDescriptor for MemPalaceComponent {
    const NAME: &'static str = "mempalace";
    type State = harmonia_mempalace::PalaceState;
    fn init() -> Self::State { harmonia_mempalace::PalaceState::new() }
    fn dispatch(state: &mut Self::State, sexp: &str) -> String {
        crate::dispatch::dispatch_mempalace(sexp, state)
    }
    fn shutdown(state: &mut Self::State) {
        let _ = harmonia_mempalace::persist(state);
    }
    fn capabilities() -> &'static [&'static str] {
        &["knowledge-graph", "aaak-compression", "tiered-context"]
    }
}

pub struct TerraphonComponent;
impl ComponentDescriptor for TerraphonComponent {
    const NAME: &'static str = "terraphon";
    type State = harmonia_terraphon::TerraphonState;
    fn init() -> Self::State { harmonia_terraphon::TerraphonState::new() }
    fn dispatch(state: &mut Self::State, sexp: &str) -> String {
        crate::dispatch::dispatch_terraphon(sexp, state)
    }
    fn capabilities() -> &'static [&'static str] {
        &["datamining", "platform-tools", "cross-node"]
    }
}

// ── Chronicle (stateless dispatch + GC tick) ────────────────────────

pub struct ChronicleComponent;
impl ComponentDescriptor for ChronicleComponent {
    const NAME: &'static str = "chronicle";
    type State = ();
    fn init() -> Self::State { let _ = harmonia_chronicle::init(); }
    fn dispatch(_state: &mut Self::State, sexp: &str) -> String {
        crate::dispatch::chronicle::dispatch(sexp)
    }
    fn tick(_state: &mut Self::State) {
        if let Ok(deleted) = harmonia_chronicle::gc() {
            if deleted > 0 {
                if let Some(obs) = harmonia_observability::get_obs_actor() {
                    if harmonia_observability::harmonia_observability_is_standard() {
                        let obs_opt: Option<ractor::ActorRef<harmonia_observability::ObsMsg>> = Some(obs.clone());
                        use harmonia_observability::Traceable;
                        obs_opt.trace_event("chronicle-gc", "tool", serde_json::json!({"rows_deleted": deleted}));
                    }
                }
            }
        }
    }
    fn capabilities() -> &'static [&'static str] { &["knowledge-base", "event-log"] }
}

// ── Ouroboros (self-healing crash ledger + patch writing) ────────────

pub struct OuroborosComponent;
impl ComponentDescriptor for OuroborosComponent {
    const NAME: &'static str = "ouroboros";
    type State = harmonia_ouroboros::OuroborosState;
    fn init() -> Self::State { harmonia_ouroboros::OuroborosState::new() }
    fn dispatch(state: &mut Self::State, sexp: &str) -> String {
        harmonia_ouroboros::dispatch(state, sexp)
    }
    fn capabilities() -> &'static [&'static str] {
        &["self-healing", "crash-ledger", "patch-writing"]
    }
}

// ── Session (actor-owned state, dispatched via ComponentDescriptor) ──

pub struct SessionComponent;
impl ComponentDescriptor for SessionComponent {
    const NAME: &'static str = "sessions";
    type State = harmonia_gateway::sessions::SessionState;
    fn init() -> Self::State { harmonia_gateway::sessions::SessionState::new() }
    fn dispatch(state: &mut Self::State, sexp: &str) -> String {
        harmonia_gateway::sessions::dispatch(state, sexp)
    }
    fn capabilities() -> &'static [&'static str] {
        &["session-management", "event-logging"]
    }
}

// ── Capability Declarations (for DynamicRegistry topic routing) ─────────
// These are NOT ComponentDescriptor impls — they're just capability data.
// Stateless/hand-written components define capabilities here for registry.

pub fn capabilities_for(name: &str) -> &'static [&'static str] {
    match name {
        "vault" => &["secret-storage", "encryption"],
        "config" => &["key-value-store"],
        "chronicle" => &["knowledge-base", "event-log"],
        "gateway" => &["baseband", "command-dispatch"],
        "tailnet" => &["mesh-network", "peer-discovery"],
        "workspace" => &["file-ops", "shell-exec"],
        "provider-router" => &["llm-routing"],
        "parallel" => &["task-execution", "tmux-agents"],
        "observability" => &["tracing", "metrics"],
        "harmonic-matrix" => &["routing-mesh", "vitruvian-scoring"],
        "memory-field" => &["field-recall", "attractor-basins", "spectral-decomposition", "dreaming"],
        "signalograd" => &["adaptive-kernel", "hebbian-learning", "reservoir-computing"],
        "ouroboros" => &["self-healing", "crash-ledger", "patch-writing"],
        "router" => &["model-selection", "tier-routing"],
        "mempalace" => MemPalaceComponent::capabilities(),
        "terraphon" => TerraphonComponent::capabilities(),
        "sessions" => SessionComponent::capabilities(),
        _ => &[],
    }
}
