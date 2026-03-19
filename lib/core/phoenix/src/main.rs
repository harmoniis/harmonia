mod config;
mod health;
mod msg;
mod subsystem;
mod supervisor;
mod trauma;

use ractor::Actor;

const COMPONENT: &str = "phoenix-core";

fn config_bool(key: &str, default: bool) -> bool {
    harmonia_config_store::get_own(COMPONENT, key)
        .ok()
        .flatten()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

#[tokio::main]
async fn main() {
    // 1. Init chronicle
    let _ = harmonia_chronicle::init();

    // 2. Prod-genesis guard
    let env_mode = harmonia_config_store::get_config_or(COMPONENT, "global", "env", "test")
        .unwrap_or_else(|_| "test".to_string());
    if env_mode.eq_ignore_ascii_case("prod") && !config_bool("allow-prod-genesis", false) {
        eprintln!(
            "[ERROR] [phoenix] Refusing to start genesis in prod without allow-prod-genesis=1"
        );
        std::process::exit(2);
    }

    // 3. Load config
    let cfg = match config::load_or_legacy() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("[WARN] [phoenix] No subsystem config: {e}");
            trauma::chronicle_record(
                "start",
                None,
                None,
                None,
                Some(&format!("env={env_mode} mode=heartbeat-only")),
            );
            eprintln!("[INFO] [phoenix] Running in heartbeat-only mode (no subsystems configured)");
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                eprintln!("[DEBUG] [phoenix] Heartbeat");
            }
        }
    };

    let health_port = cfg.health_port;
    let n_subsystems = cfg.subsystems.len();

    trauma::chronicle_record(
        "start",
        None,
        None,
        None,
        Some(&format!(
            "env={env_mode} subsystems={n_subsystems} health_port={health_port}"
        )),
    );
    eprintln!(
        "[INFO] [phoenix] Supervisor online (env={env_mode}, subsystems={n_subsystems}, health=:{health_port})"
    );

    // 4. Spawn supervisor actor
    let (supervisor_ref, supervisor_handle) = Actor::spawn(
        Some("phoenix-supervisor".to_string()),
        supervisor::PhoenixSupervisor,
        cfg,
    )
    .await
    .expect("failed to spawn PhoenixSupervisor actor");

    // 5. Spawn health server
    let health_sup = supervisor_ref.clone();
    tokio::spawn(async move {
        health::serve(health_port, health_sup).await;
    });

    // Write pidfile
    let pidfile_path = format!("{}/phoenix.pid", trauma::state_root());
    if let Some(parent) = std::path::Path::new(&pidfile_path).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(&pidfile_path, std::process::id().to_string());

    // 6. Wait for SIGTERM/SIGINT → send Shutdown
    let shutdown_sup = supervisor_ref.clone();
    tokio::spawn(async move {
        let mut sigterm =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).unwrap();
        let sigint = tokio::signal::ctrl_c();

        tokio::select! {
            _ = sigterm.recv() => {
                eprintln!("[INFO] [phoenix] Received SIGTERM");
            }
            _ = sigint => {
                eprintln!("[INFO] [phoenix] Received SIGINT");
            }
        }

        let _ = shutdown_sup.cast(msg::SupervisorMsg::Shutdown);
    });

    // 7. Await supervisor exit
    supervisor_handle.await.unwrap();
    let _ = std::fs::remove_file(&pidfile_path);
    eprintln!("[INFO] [phoenix] Supervisor exited, shutting down");
}

#[cfg(test)]
mod tests {
    #[test]
    fn phoenix_test_harness_runs() {
        assert_eq!(super::config_bool("does-not-exist", true), true);
    }
}
