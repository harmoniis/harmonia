//! Actor spawning, supervisor registration, and component registry setup.

use std::collections::HashMap;

use ractor::Actor;

use crate::actors::{self, ComponentMsg, MatrixMsg};
use crate::bridge;
use crate::msg;
use crate::supervisor;
use harmonia_observability::ObsMsg;

/// All actor refs produced by the spawn phase, needed by later stages.
pub struct SpawnedActors {
    pub supervisor_ref: ractor::ActorRef<msg::RuntimeMsg>,
    pub supervisor_handle: tokio::task::JoinHandle<()>,
    pub obs_ref: ractor::ActorRef<ObsMsg>,
    pub bridge_ref: ractor::ActorRef<msg::BridgeMsg>,
    pub chronicle_ref: ractor::ActorRef<ComponentMsg>,
    pub gateway_ref: ractor::ActorRef<ComponentMsg>,
    pub tailnet_ref: ractor::ActorRef<ComponentMsg>,
    pub signalograd_ref: ractor::ActorRef<ComponentMsg>,
    pub memory_field_ref: ractor::ActorRef<ComponentMsg>,
    pub harmonic_matrix_ref: ractor::ActorRef<MatrixMsg>,
    pub vault_ref: ractor::ActorRef<ComponentMsg>,
    pub config_ref: ractor::ActorRef<ComponentMsg>,
    pub provider_router_ref: ractor::ActorRef<ComponentMsg>,
    pub parallel_ref: ractor::ActorRef<ComponentMsg>,
    pub router_ref: ractor::ActorRef<ComponentMsg>,
    pub mempalace_ref: ractor::ActorRef<ComponentMsg>,
    pub terraphon_ref: ractor::ActorRef<ComponentMsg>,
    pub ouroboros_ref: ractor::ActorRef<ComponentMsg>,
    pub session_ref: ractor::ActorRef<ComponentMsg>,
    pub dynamic_registry: crate::dynamic_registry::SharedDynamicRegistry,
    pub topic_bus: crate::topic_bus::SharedTopicBus,
}

/// Spawn all actors, register with supervisor, and build component registry.
pub async fn spawn_all(module_registry: HashMap<String, crate::registry::ModuleEntry>) -> SpawnedActors {
    // 1. Spawn ObservabilityActor FIRST -- other actors receive its ref
    let obs_sender = harmonia_observability::start_sender();
    let obs_config = harmonia_observability::get_config().cloned();
    let (obs_ref, _obs_handle) = Actor::spawn(
        Some("observability".to_string()),
        actors::ObservabilityActor,
        (obs_sender, obs_config),
    )
    .await
    .expect("observability actor is required for runtime boot");
    harmonia_observability::set_obs_actor(obs_ref.clone());
    let obs_opt: Option<ractor::ActorRef<ObsMsg>> = Some(obs_ref.clone());

    // 2. Spawn SbclBridgeActor (not a component -- standalone)
    let (bridge_ref, _bridge_handle) =
        Actor::spawn(Some("sbcl-bridge".to_string()), bridge::SbclBridgeActor, ())
            .await
            .expect("sbcl-bridge actor is required for runtime boot");

    // 3. Spawn supervisor, then link all components to it
    let (supervisor_ref, supervisor_handle) = Actor::spawn(
        Some("runtime-supervisor".to_string()),
        supervisor::RuntimeSupervisor,
        (bridge_ref.clone(), module_registry),
    )
    .await
    .expect("runtime-supervisor actor is required for runtime boot");

    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterObsActor(obs_ref.clone()));

    // 4. Spawn component actors linked to supervisor
    let chronicle_ref = spawn_linked("chronicle", actors::ChronicleComponentActor, (), &supervisor_ref).await;
    let gateway_ref = spawn_linked("gateway", actors::GatewayActor, (bridge_ref.clone(), obs_opt.clone()), &supervisor_ref).await;
    let tailnet_ref = spawn_linked("tailnet", actors::TailnetActor, (bridge_ref.clone(), obs_opt.clone()), &supervisor_ref).await;
    let signalograd_ref = spawn_linked("signalograd", actors::SignalogradActor, (bridge_ref.clone(), obs_opt.clone()), &supervisor_ref).await;
    let memory_field_ref = spawn_linked("memory-field", actors::MemoryFieldActor, (bridge_ref.clone(), obs_opt.clone()), &supervisor_ref).await;
    let harmonic_matrix_ref = spawn_linked("harmonic-matrix", actors::HarmonicMatrixActor, (), &supervisor_ref).await;
    let vault_ref = spawn_linked("vault", actors::VaultActor, obs_opt.clone(), &supervisor_ref).await;
    let config_ref = spawn_linked("config", actors::ConfigActor, (), &supervisor_ref).await;
    let workspace_ref = spawn_linked("workspace", actors::WorkspaceActor, (), &supervisor_ref).await;
    let provider_router_ref = spawn_linked("provider-router", actors::ProviderRouterActor, (), &supervisor_ref).await;
    let parallel_ref = spawn_linked("parallel", actors::ParallelActor, (), &supervisor_ref).await;
    let router_ref = spawn_linked("router", actors::RouterActor, obs_opt.clone(), &supervisor_ref).await;
    let mempalace_ref = spawn_linked("mempalace", actors::MemPalaceActor, (), &supervisor_ref).await;
    let terraphon_ref = spawn_linked("terraphon", actors::TerraphonActor, (), &supervisor_ref).await;
    let ouroboros_ref = spawn_linked("ouroboros", actors::OuroborosActor, (), &supervisor_ref).await;
    let session_ref = spawn_linked("sessions", actors::SessionActor, (), &supervisor_ref).await;

    // 5. Register component actors with supervisor for restart tracking
    register_component(&supervisor_ref, "chronicle", &chronicle_ref);
    register_component(&supervisor_ref, "gateway", &gateway_ref);
    register_component(&supervisor_ref, "tailnet", &tailnet_ref);
    register_component(&supervisor_ref, "signalograd", &signalograd_ref);
    register_component(&supervisor_ref, "memory-field", &memory_field_ref);
    register_component(&supervisor_ref, "vault", &vault_ref);
    register_component(&supervisor_ref, "config", &config_ref);
    register_component(&supervisor_ref, "provider-router", &provider_router_ref);
    register_component(&supervisor_ref, "parallel", &parallel_ref);
    register_component(&supervisor_ref, "router", &router_ref);
    register_component(&supervisor_ref, "mempalace", &mempalace_ref);
    register_component(&supervisor_ref, "terraphon", &terraphon_ref);
    register_component(&supervisor_ref, "ouroboros", &ouroboros_ref);
    register_component(&supervisor_ref, "sessions", &session_ref);
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterMatrixActor(harmonic_matrix_ref.clone()));

    // 6. Build dynamic registry (pluggable, HashMap-based) + topic bus
    let dyn_reg = crate::dynamic_registry::new_dynamic();
    let topic_bus = crate::topic_bus::new_topic_bus();
    let all_actors: &[(&str, &ractor::ActorRef<ComponentMsg>)] = &[
        ("chronicle", &chronicle_ref), ("gateway", &gateway_ref),
        ("tailnet", &tailnet_ref), ("signalograd", &signalograd_ref),
        ("memory-field", &memory_field_ref), ("vault", &vault_ref),
        ("config", &config_ref), ("provider-router", &provider_router_ref),
        ("parallel", &parallel_ref), ("router", &router_ref),
        ("workspace", &workspace_ref), ("mempalace", &mempalace_ref),
        ("terraphon", &terraphon_ref),
        ("ouroboros", &ouroboros_ref),
        ("sessions", &session_ref),
    ];
    for &(name, actor_ref) in all_actors {
        let caps = crate::components::capabilities_for(name);
        dyn_reg.register(name, actor_ref.clone(), caps);
        for cap in caps { topic_bus.subscribe(cap, actor_ref.clone()); }
    }
    eprintln!("[INFO] [runtime] DynamicRegistry: {} components, TopicBus: {} topics",
        dyn_reg.len(), topic_bus.topics().len());

    // Inject DynamicRegistry + TopicBus into supervisor for crash-restart handling.
    let _ = supervisor_ref.cast(msg::RuntimeMsg::SetDynamicRegistry(dyn_reg.clone()));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::SetTopicBus(topic_bus.clone()));

    SpawnedActors {
        supervisor_ref,
        supervisor_handle,
        obs_ref,
        bridge_ref,
        chronicle_ref,
        gateway_ref,
        tailnet_ref,
        signalograd_ref,
        memory_field_ref,
        harmonic_matrix_ref,
        vault_ref,
        config_ref,
        provider_router_ref,
        parallel_ref,
        router_ref,
        mempalace_ref,
        terraphon_ref,
        ouroboros_ref,
        session_ref,
        dynamic_registry: dyn_reg,
        topic_bus,
    }
}

/// Spawn an actor linked to the supervisor, returning its ActorRef.
///
/// Component actors are critical for system integrity. If a spawn fails,
/// the error is logged and propagated as a panic — the supervisor itself
/// will handle restart semantics for the entire runtime.
async fn spawn_linked<A, M, Args>(
    name: &str,
    actor: A,
    args: Args,
    supervisor_ref: &ractor::ActorRef<msg::RuntimeMsg>,
) -> ractor::ActorRef<M>
where
    A: Actor<Msg = M, Arguments = Args, State: Send>,
    M: ractor::Message,
    Args: Send,
{
    match Actor::spawn_linked(
        Some(name.to_string()),
        actor,
        args,
        supervisor_ref.get_cell(),
    )
    .await
    {
        Ok((actor_ref, _handle)) => actor_ref,
        Err(e) => {
            eprintln!("[FATAL] [runtime] failed to spawn component {name}: {e}");
            panic!("component {name} is required for runtime boot: {e}");
        }
    }
}

fn register_component(
    supervisor_ref: &ractor::ActorRef<msg::RuntimeMsg>,
    name: &str,
    actor_ref: &ractor::ActorRef<ComponentMsg>,
) {
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        name.to_string(),
        actor_ref.clone(),
    ));
}

/// Run the coordinated shutdown sequence.
pub async fn shutdown(
    supervisor_ref: ractor::ActorRef<msg::RuntimeMsg>,
    obs_ref: ractor::ActorRef<ObsMsg>,
    bridge_ref: ractor::ActorRef<msg::BridgeMsg>,
    component_actors: Vec<ractor::ActorRef<ComponentMsg>>,
    matrix_ref: ractor::ActorRef<MatrixMsg>,
    state_root: String,
) {
    // 1. Shutdown all component actors first (they produce trace events)
    for actor in &component_actors {
        let _ = actor.cast(ComponentMsg::Shutdown);
    }
    let _ = matrix_ref.cast(MatrixMsg::Shutdown);

    // 2. Give components time to drain, then drain bridge
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let _ = ractor::call_t!(bridge_ref, msg::BridgeMsg::Drain, 5000);

    // 3. Shutdown observability LAST (captures shutdown traces)
    let _ = obs_ref.cast(ObsMsg::Shutdown);
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    // 4. Shutdown supervisor
    let _ = supervisor_ref.cast(msg::RuntimeMsg::Shutdown);

    // 5. Hard deadline with tokio::time::timeout (no process::exit)
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    eprintln!("[WARN] [runtime] Shutdown timeout — forcing exit");
    // Clean up IPC artifacts before exit
    let _ = std::fs::remove_file(format!("{}/ipc.token", state_root));
    let _ = std::fs::remove_file(format!("{}/ipc.name", state_root));
    std::process::exit(0);
}
