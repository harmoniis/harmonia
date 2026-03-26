mod actors;
mod bridge;
mod dispatch;
mod init;
mod ipc;
mod msg;
mod registry;
mod supervisor;

use std::env;

use ractor::Actor;

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

#[tokio::main]
async fn main() {
    eprintln!("[INFO] [runtime] harmonia-runtime starting");

    // 0. Bootstrap core infrastructure — config-store and vault must be
    //    initialized before any module validation or actor startup.
    //    Without this, vault lookups silently fall back to /tmp/harmonia/
    //    if HARMONIA_STATE_ROOT or HARMONIA_VAULT_DB are unset.
    if let Err(e) = harmonia_config_store::init_v2() {
        eprintln!("[WARN] [runtime] config-store init failed: {e}");
    }
    if let Err(e) = harmonia_vault::init_from_env() {
        eprintln!("[WARN] [runtime] vault init failed: {e}");
    }

    // Log resolved paths for debuggability
    eprintln!(
        "[INFO] [runtime] config-db: {}",
        harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", "(default)")
            .unwrap_or_else(|_| "(error)".into())
    );
    eprintln!(
        "[INFO] [runtime] vault-db: {}",
        harmonia_vault::store_path().display()
    );

    // 1. Initialize only enabled components (config-driven)
    let module_registry = init::init_all();

    // 2. Determine socket path
    let socket_path = env::var("HARMONIA_RUNTIME_SOCKET")
        .unwrap_or_else(|_| format!("{}/runtime.sock", state_root()));

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

    // 4. Spawn SbclBridgeActor
    let (bridge_ref, _bridge_handle) =
        Actor::spawn(Some("sbcl-bridge".to_string()), bridge::SbclBridgeActor, ())
            .await
            .expect("failed to spawn SbclBridgeActor");

    // 5. Spawn component actors — all receive obs ref
    let (chronicle_ref, _) =
        Actor::spawn(Some("chronicle".to_string()), actors::ChronicleActor, ())
            .await
            .expect("failed to spawn ChronicleActor");

    let (gateway_ref, _) = Actor::spawn(
        Some("gateway".to_string()),
        actors::GatewayActor,
        (bridge_ref.clone(), obs_opt.clone()),
    )
    .await
    .expect("failed to spawn GatewayActor");

    let (tailnet_ref, _) = Actor::spawn(
        Some("tailnet".to_string()),
        actors::TailnetActor,
        (bridge_ref.clone(), obs_opt.clone()),
    )
    .await
    .expect("failed to spawn TailnetActor");

    let (signalograd_ref, _) = Actor::spawn(
        Some("signalograd".to_string()),
        actors::SignalogradActor,
        (bridge_ref.clone(), obs_opt.clone()),
    )
    .await
    .expect("failed to spawn SignalogradActor");

    let (memory_field_ref, _) = Actor::spawn(
        Some("memory-field".to_string()),
        actors::MemoryFieldActor,
        (bridge_ref.clone(), obs_opt.clone()),
    )
    .await
    .expect("failed to spawn MemoryFieldActor");

    let (harmonic_matrix_ref, _) = Actor::spawn(
        Some("harmonic-matrix".to_string()),
        actors::HarmonicMatrixActor,
        (),
    )
    .await
    .expect("failed to spawn HarmonicMatrixActor");

    let (vault_ref, _) = Actor::spawn(
        Some("vault".to_string()),
        actors::VaultActor,
        obs_opt.clone(),
    )
    .await
    .expect("failed to spawn VaultActor");

    let (config_ref, _) = Actor::spawn(Some("config".to_string()), actors::ConfigActor, ())
        .await
        .expect("failed to spawn ConfigActor");

    let (provider_router_ref, _) = Actor::spawn(
        Some("provider-router".to_string()),
        actors::ProviderRouterActor,
        (),
    )
    .await
    .expect("failed to spawn ProviderRouterActor");

    let (parallel_ref, _) = Actor::spawn(Some("parallel".to_string()), actors::ParallelActor, ())
        .await
        .expect("failed to spawn ParallelActor");

    let (router_ref, _) = Actor::spawn(
        Some("router".to_string()),
        actors::RouterActor,
        obs_opt.clone(),
    )
    .await
    .expect("failed to spawn RouterActor");

    // Store matrix actor ref for dispatch routing
    let matrix_for_supervisor = harmonic_matrix_ref.clone();

    // 6. Spawn RuntimeSupervisor (with module registry)
    let (supervisor_ref, supervisor_handle) = Actor::spawn(
        Some("runtime-supervisor".to_string()),
        supervisor::RuntimeSupervisor,
        (bridge_ref, module_registry),
    )
    .await
    .expect("failed to spawn RuntimeSupervisor");

    // 6b. Register component actors with the supervisor for restart tracking
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
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterMatrixActor(matrix_for_supervisor));
    let _ = supervisor_ref.cast(msg::RuntimeMsg::RegisterObsActor(obs_ref.clone()));

    eprintln!("[INFO] [runtime] All actors spawned, starting IPC server");

    // 7. Spawn IPC listener
    let ipc_sup = supervisor_ref.clone();
    let ipc_path = socket_path.clone();
    tokio::spawn(async move {
        ipc::serve(&ipc_path, ipc_sup).await;
    });

    // 8. Spawn tick loop — drives periodic polling for all component actors
    let tick_actors = vec![
        chronicle_ref.clone(),
        gateway_ref.clone(),
        tailnet_ref.clone(),
        signalograd_ref.clone(),
        memory_field_ref.clone(),
        router_ref.clone(),
    ];
    let tick_matrix = harmonic_matrix_ref.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        loop {
            interval.tick().await;
            for actor in &tick_actors {
                let _ = actor.cast(ComponentMsg::Tick);
            }
            let _ = tick_matrix.cast(actors::MatrixMsg::Tick);
        }
    });

    // 9. Wait for SIGTERM/SIGINT → coordinated shutdown with timeout
    let shutdown_sup = supervisor_ref.clone();
    let shutdown_obs = obs_ref;
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
    tokio::spawn(async move {
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

        // Shutdown observability actor first (flushes traces)
        let _ = shutdown_obs.cast(ObsMsg::Shutdown);

        // Shutdown all component actors
        for actor in &shutdown_actors {
            let _ = actor.cast(ComponentMsg::Shutdown);
        }
        let _ = shutdown_matrix.cast(actors::MatrixMsg::Shutdown);

        // Give components 2 seconds to finish, then shutdown supervisor
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        let _ = shutdown_sup.cast(msg::RuntimeMsg::Shutdown);

        // Hard deadline: if supervisor doesn't stop within 5 more seconds, force exit
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        eprintln!("[WARN] [runtime] Shutdown timeout — forcing exit");
        std::process::exit(0);
    });

    // 10. Await supervisor exit
    let _ = supervisor_handle.await;

    // 11. Clean up socket file
    let _ = std::fs::remove_file(&socket_path);
    eprintln!("[INFO] [runtime] harmonia-runtime exited");
}
