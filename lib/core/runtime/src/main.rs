mod actors;
mod bridge;
mod component_registry;
mod dispatch;
mod init;
mod ipc;
mod msg;
mod registry;
mod supervisor;

use std::env;
use std::sync::Arc;

use ractor::Actor;
use rand::Rng;
use tokio::sync::Notify;

use actors::ComponentMsg;
use harmonia_observability::ObsMsg;

const COMPONENT: &str = "harmonia-runtime";

fn state_root() -> String {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or_else(|_| default)
}

/// Generate a 32-byte random token as 64 hex characters.
fn generate_token() -> String {
    let bytes: [u8; 32] = rand::thread_rng().gen();
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Write the IPC token to disk for CLI clients in separate shells.
fn persist_token(state_root: &str, token: &str) {
    let token_path = format!("{}/ipc.token", state_root);
    if let Some(parent) = std::path::Path::new(&token_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&token_path, token);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&token_path, std::fs::Permissions::from_mode(0o600));
    }
}

/// Write the IPC name to disk for SBCL and CLI clients.
fn persist_ipc_name(state_root: &str, name: &str) {
    let name_path = format!("{}/ipc.name", state_root);
    let _ = std::fs::write(&name_path, name);
}

#[tokio::main]
async fn main() {
    eprintln!("[INFO] [runtime] harmonia-runtime starting");

    // 0. Bootstrap core infrastructure
    if let Err(e) = harmonia_config_store::init_v2() {
        eprintln!("[WARN] [runtime] config-store init failed: {e}");
    }
    if let Err(e) = harmonia_vault::init_from_env() {
        eprintln!("[WARN] [runtime] vault init failed: {e}");
    }

    let sr = state_root();
    eprintln!("[INFO] [runtime] state-root: {sr}");
    eprintln!(
        "[INFO] [runtime] vault-db: {}",
        harmonia_vault::store_path().display()
    );

    // 1. Initialize modules (config-driven)
    let module_registry = init::init_all();

    // 2. Compute IPC name and security token
    let ipc_name = env::var("HARMONIA_RUNTIME_SOCKET")
        .unwrap_or_else(|_| ipc::ipc_name(&sr));
    let token = Arc::new(generate_token());
    env::set_var("HARMONIA_IPC_TOKEN", token.as_str());
    persist_token(&sr, &token);
    persist_ipc_name(&sr, &ipc_name);

    // 3. Spawn ObservabilityActor FIRST — other actors receive its ref
    let obs_sender = harmonia_observability::start_sender();
    let obs_config = harmonia_observability::get_config().cloned();
    let (obs_ref, _obs_handle) = Actor::spawn(
        Some("observability".to_string()),
        actors::ObservabilityActor,
        (obs_sender, obs_config),
    )
    .await
    .expect("failed to spawn ObservabilityActor");
    harmonia_observability::set_obs_actor(obs_ref.clone());
    let obs_opt: Option<ractor::ActorRef<ObsMsg>> = Some(obs_ref.clone());

    // 4. Spawn SbclBridgeActor (not a component — standalone)
    let (bridge_ref, _bridge_handle) =
        Actor::spawn(Some("sbcl-bridge".to_string()), bridge::SbclBridgeActor, ())
            .await
            .expect("failed to spawn SbclBridgeActor");

    // 5. Spawn component actors with spawn_linked to supervisor for proper supervision.
    //    The supervisor is spawned first, then components are linked to it.
    let (supervisor_ref, supervisor_handle) = Actor::spawn(
        Some("runtime-supervisor".to_string()),
        supervisor::RuntimeSupervisor,
        (bridge_ref.clone(), module_registry),
    )
    .await
    .expect("failed to spawn RuntimeSupervisor");

    // Register obs actor with supervisor
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterObsActor(obs_ref.clone()));

    // 5b. Spawn component actors linked to the supervisor
    let chronicle_ref = Actor::spawn_linked(
        Some("chronicle".to_string()),
        actors::ChronicleActor,
        (),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn ChronicleActor")
    .0;

    let gateway_ref = Actor::spawn_linked(
        Some("gateway".to_string()),
        actors::GatewayActor,
        (bridge_ref.clone(), obs_opt.clone()),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn GatewayActor")
    .0;

    let tailnet_ref = Actor::spawn_linked(
        Some("tailnet".to_string()),
        actors::TailnetActor,
        (bridge_ref.clone(), obs_opt.clone()),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn TailnetActor")
    .0;

    let signalograd_ref = Actor::spawn_linked(
        Some("signalograd".to_string()),
        actors::SignalogradActor,
        (bridge_ref.clone(), obs_opt.clone()),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn SignalogradActor")
    .0;

    let memory_field_ref = Actor::spawn_linked(
        Some("memory-field".to_string()),
        actors::MemoryFieldActor,
        (bridge_ref.clone(), obs_opt.clone()),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn MemoryFieldActor")
    .0;

    let harmonic_matrix_ref = Actor::spawn_linked(
        Some("harmonic-matrix".to_string()),
        actors::HarmonicMatrixActor,
        (),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn HarmonicMatrixActor")
    .0;

    let vault_ref = Actor::spawn_linked(
        Some("vault".to_string()),
        actors::VaultActor,
        obs_opt.clone(),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn VaultActor")
    .0;

    let config_ref = Actor::spawn_linked(
        Some("config".to_string()),
        actors::ConfigActor,
        (),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn ConfigActor")
    .0;

    let provider_router_ref = Actor::spawn_linked(
        Some("provider-router".to_string()),
        actors::ProviderRouterActor,
        (),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn ProviderRouterActor")
    .0;

    let parallel_ref = Actor::spawn_linked(
        Some("parallel".to_string()),
        actors::ParallelActor,
        (),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn ParallelActor")
    .0;

    let router_ref = Actor::spawn_linked(
        Some("router".to_string()),
        actors::RouterActor,
        obs_opt.clone(),
        supervisor_ref.get_cell(),
    )
    .await
    .expect("failed to spawn RouterActor")
    .0;

    // 6. Register component actors with supervisor for restart tracking
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "chronicle".to_string(),
        chronicle_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "gateway".to_string(),
        gateway_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "tailnet".to_string(),
        tailnet_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "signalograd".to_string(),
        signalograd_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "memory-field".to_string(),
        memory_field_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "vault".to_string(),
        vault_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "config".to_string(),
        config_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "provider-router".to_string(),
        provider_router_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "parallel".to_string(),
        parallel_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterComponent(
        "router".to_string(),
        router_ref.clone(),
    ));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterMatrixActor(
        harmonic_matrix_ref.clone(),
    ));

    // 7. Build shared component registry for direct IPC dispatch (lock-free).
    let registry = component_registry::new();
    component_registry::register(&registry, "chronicle", chronicle_ref.clone());
    component_registry::register(&registry, "gateway", gateway_ref.clone());
    component_registry::register(&registry, "tailnet", tailnet_ref.clone());
    component_registry::register(&registry, "signalograd", signalograd_ref.clone());
    component_registry::register(&registry, "memory-field", memory_field_ref.clone());
    component_registry::register(&registry, "vault", vault_ref.clone());
    component_registry::register(&registry, "config", config_ref.clone());
    component_registry::register(&registry, "provider-router", provider_router_ref.clone());
    component_registry::register(&registry, "parallel", parallel_ref.clone());
    component_registry::register(&registry, "router", router_ref.clone());

    // 8. Readiness gate — IPC server waits until all actors are registered
    let ready = Arc::new(Notify::new());

    // 9. Spawn IPC listener (cross-platform: Unix sockets / Windows named pipes)
    let ipc_sup = supervisor_ref.clone();
    let ipc_name_owned = ipc_name.clone();
    let ipc_reg = registry.clone();
    let ipc_token = token.clone();
    let ipc_ready = ready.clone();
    tokio::spawn(async move {
        ipc::serve(&ipc_name_owned, ipc_sup, ipc_reg, ipc_token, ipc_ready).await;
    });

    // Wait for IPC listener to be ready before starting tick loops
    ready.notified().await;
    eprintln!("[INFO] [runtime] All actors spawned, IPC ready");

    // 10. Per-actor tick loops — each actor ticks at its own cadence (no thundering herd)
    spawn_tick(chronicle_ref.clone(), std::time::Duration::from_secs(60));
    spawn_tick(gateway_ref.clone(), std::time::Duration::from_secs(2));
    spawn_tick(tailnet_ref.clone(), std::time::Duration::from_secs(3));
    spawn_tick(signalograd_ref.clone(), std::time::Duration::from_secs(10));
    spawn_tick(memory_field_ref.clone(), std::time::Duration::from_secs(5));
    spawn_tick(router_ref.clone(), std::time::Duration::from_secs(10));
    // Matrix actor has its own message type
    spawn_matrix_tick(harmonic_matrix_ref.clone(), std::time::Duration::from_secs(5));

    // 11. Wait for shutdown signal → coordinated drain
    let shutdown_sup = supervisor_ref.clone();
    let shutdown_obs = obs_ref;
    let shutdown_bridge = bridge_ref;
    let shutdown_actors = vec![
        chronicle_ref,
        gateway_ref,
        tailnet_ref,
        signalograd_ref,
        memory_field_ref,
        vault_ref,
        config_ref,
        provider_router_ref,
        parallel_ref,
        router_ref,
    ];
    let shutdown_matrix = harmonic_matrix_ref;
    let shutdown_sr = sr.clone();
    tokio::spawn(async move {
        wait_for_shutdown_signal().await;

        // 1. Shutdown all component actors first (they produce trace events)
        for actor in &shutdown_actors {
            let _ = actor.cast(ComponentMsg::Shutdown);
        }
        let _ = shutdown_matrix.cast(actors::MatrixMsg::Shutdown);

        // 2. Give components time to drain, then drain bridge
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let _ = ractor::call_t!(shutdown_bridge, msg::BridgeMsg::Drain, 5000);

        // 3. Shutdown observability LAST (captures shutdown traces)
        let _ = shutdown_obs.cast(ObsMsg::Shutdown);
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;

        // 4. Shutdown supervisor
        let _ = shutdown_sup.cast(msg::RuntimeMsg::Shutdown);

        // 5. Hard deadline with tokio::time::timeout (no process::exit)
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        eprintln!("[WARN] [runtime] Shutdown timeout — forcing exit");
        // Clean up IPC artifacts before exit
        let _ = std::fs::remove_file(format!("{}/ipc.token", shutdown_sr));
        let _ = std::fs::remove_file(format!("{}/ipc.name", shutdown_sr));
        std::process::exit(0);
    });

    // 12. Await supervisor exit
    let _ = supervisor_handle.await;

    // 13. Clean up IPC artifacts
    let _ = std::fs::remove_file(format!("{}/ipc.token", sr));
    let _ = std::fs::remove_file(format!("{}/ipc.name", sr));
    eprintln!("[INFO] [runtime] harmonia-runtime exited");
}

/// Spawn a per-actor tick loop at the given interval.
fn spawn_tick(actor: ractor::ActorRef<ComponentMsg>, interval: std::time::Duration) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(interval);
        loop {
            timer.tick().await;
            let _ = actor.cast(ComponentMsg::Tick);
        }
    });
}

/// Spawn a tick loop for the matrix actor (separate message type).
fn spawn_matrix_tick(actor: ractor::ActorRef<actors::MatrixMsg>, interval: std::time::Duration) {
    tokio::spawn(async move {
        let mut timer = tokio::time::interval(interval);
        loop {
            timer.tick().await;
            let _ = actor.cast(actors::MatrixMsg::Tick);
        }
    });
}

/// Cross-platform shutdown signal handler.
async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        let sigint = tokio::signal::ctrl_c();
        tokio::select! {
            _ = sigterm.recv() => {
                eprintln!("[INFO] [runtime] Received SIGTERM");
            }
            _ = sigint => {
                eprintln!("[INFO] [runtime] Received SIGINT");
            }
        }
    }
    #[cfg(not(unix))]
    {
        // Windows: only CTRL_C is available
        let _ = tokio::signal::ctrl_c().await;
        eprintln!("[INFO] [runtime] Received Ctrl-C");
    }
}
