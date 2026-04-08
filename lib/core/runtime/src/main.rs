#[macro_use]
mod macros;
mod actors;
mod bridge;
mod components;
mod dispatch;
mod dynamic_registry;
mod topic_bus;
mod hardening;
mod init;
mod ipc;
mod msg;
mod registry;
mod signal;
mod spawn;
mod supervisor;
mod tick;
mod token;

use std::env;
use std::sync::Arc;

use tokio::sync::Notify;

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

    // 0. Harden + bootstrap infrastructure
    hardening::harden();
    if let Err(e) = harmonia_config_store::init_v2() {
        eprintln!("[WARN] [runtime] config-store init failed: {e}");
    }
    if let Err(e) = harmonia_vault::init_from_env() {
        eprintln!("[WARN] [runtime] vault init failed: {e}");
    }
    let sr = state_root();
    eprintln!("[INFO] [runtime] state-root: {sr}");
    eprintln!("[INFO] [runtime] vault-db: {}", harmonia_vault::store_path().display());

    // 1. Init modules, compute IPC name, generate token
    let module_registry = init::init_all();
    let ipc_name = env::var("HARMONIA_RUNTIME_SOCKET")
        .unwrap_or_else(|_| ipc::ipc_name(&sr));
    let ipc_token = Arc::new(token::generate());
    env::set_var("HARMONIA_IPC_TOKEN", ipc_token.as_str());
    token::persist_token(&sr, &ipc_token);
    token::persist_ipc_name(&sr, &ipc_name);

    // 2. Spawn all actors
    let spawned = spawn::spawn_all(module_registry).await;

    // 3. IPC listener
    let ready = Arc::new(Notify::new());
    let ipc_sup = spawned.supervisor_ref.clone();
    let ipc_reg = spawned.dynamic_registry.clone();
    let ipc_bus = spawned.topic_bus.clone();
    let ipc_ready = ready.clone();
    let ipc_name_owned = ipc_name.clone();
    let ipc_token_clone = ipc_token.clone();
    tokio::spawn(async move {
        ipc::serve(&ipc_name_owned, ipc_sup, ipc_reg, ipc_bus, ipc_token_clone, ipc_ready).await;
    });
    ready.notified().await;
    eprintln!("[INFO] [runtime] All actors spawned, IPC ready");

    // 4. Per-actor tick loops
    tick::spawn_tick(spawned.chronicle_ref.clone(), std::time::Duration::from_secs(60));
    tick::spawn_tick(spawned.gateway_ref.clone(), std::time::Duration::from_secs(2));
    tick::spawn_tick(spawned.tailnet_ref.clone(), std::time::Duration::from_secs(3));
    tick::spawn_tick(spawned.signalograd_ref.clone(), std::time::Duration::from_secs(10));
    tick::spawn_tick(spawned.memory_field_ref.clone(), std::time::Duration::from_secs(5));
    tick::spawn_tick(spawned.router_ref.clone(), std::time::Duration::from_secs(10));
    tick::spawn_matrix_tick(spawned.harmonic_matrix_ref.clone(), std::time::Duration::from_secs(5));

    // 5. Shutdown handler
    let shutdown_actors = vec![
        spawned.chronicle_ref,
        spawned.gateway_ref,
        spawned.tailnet_ref,
        spawned.signalograd_ref,
        spawned.memory_field_ref,
        spawned.vault_ref,
        spawned.config_ref,
        spawned.provider_router_ref,
        spawned.parallel_ref,
        spawned.router_ref,
        spawned.mempalace_ref,
        spawned.terraphon_ref,
        spawned.ouroboros_ref,
    ];
    let shutdown_sup = spawned.supervisor_ref.clone();
    let shutdown_obs = spawned.obs_ref;
    let shutdown_bridge = spawned.bridge_ref;
    let shutdown_matrix = spawned.harmonic_matrix_ref;
    let shutdown_sr = sr.clone();
    tokio::spawn(async move {
        signal::wait_for_shutdown_signal().await;
        spawn::shutdown(shutdown_sup, shutdown_obs, shutdown_bridge, shutdown_actors, shutdown_matrix, shutdown_sr).await;
    });

    // 6. Await supervisor exit + cleanup
    let _ = spawned.supervisor_handle.await;
    let _ = std::fs::remove_file(format!("{}/ipc.token", sr));
    let _ = std::fs::remove_file(format!("{}/ipc.name", sr));
    eprintln!("[INFO] [runtime] harmonia-runtime exited");
}
