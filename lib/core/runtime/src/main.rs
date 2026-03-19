mod bridge;
mod ipc;
mod msg;
mod supervisor;

use std::env;

use ractor::Actor;

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
    // 1. Init chronicle
    let _ = harmonia_chronicle::init();

    eprintln!("[INFO] [runtime] harmonia-runtime starting");

    // 2. Determine socket path
    let socket_path = env::var("HARMONIA_RUNTIME_SOCKET")
        .unwrap_or_else(|_| format!("{}/runtime.sock", state_root()));

    // 3. Spawn SbclBridgeActor
    let (bridge_ref, _bridge_handle) =
        Actor::spawn(Some("sbcl-bridge".to_string()), bridge::SbclBridgeActor, ())
            .await
            .expect("failed to spawn SbclBridgeActor");

    // 4. Spawn RuntimeSupervisor (linked to bridge)
    let (supervisor_ref, supervisor_handle) = Actor::spawn(
        Some("runtime-supervisor".to_string()),
        supervisor::RuntimeSupervisor,
        bridge_ref,
    )
    .await
    .expect("failed to spawn RuntimeSupervisor");

    eprintln!("[INFO] [runtime] Actors spawned, starting IPC server");

    // 5. Spawn IPC listener
    let ipc_sup = supervisor_ref.clone();
    let ipc_path = socket_path.clone();
    tokio::spawn(async move {
        ipc::serve(&ipc_path, ipc_sup).await;
    });

    // 6. Wait for SIGTERM/SIGINT → send Shutdown
    let shutdown_sup = supervisor_ref.clone();
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

        let _ = shutdown_sup.cast(msg::RuntimeMsg::Shutdown);
    });

    // 7. Await supervisor exit
    supervisor_handle.await.unwrap();

    // 8. Clean up socket file
    let _ = std::fs::remove_file(&socket_path);
    eprintln!("[INFO] [runtime] harmonia-runtime exited");
}
