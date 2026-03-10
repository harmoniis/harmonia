use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

const COMPONENT: &str = "phoenix-core";

fn config_bool(key: &str, default: bool) -> bool {
    harmonia_config_store::get_own(COMPONENT, key)
        .ok()
        .flatten()
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn state_root() -> String {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or_else(|_| default)
}

fn append_trauma(line: &str) {
    let trauma_path =
        env::var("PHOENIX_TRAUMA_LOG").unwrap_or_else(|_| format!("{}/trauma.log", state_root()));
    if let Some(parent) = std::path::Path::new(&trauma_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&trauma_path)
    {
        let _ = writeln!(f, "{line}");
    }

    let recovery_path = harmonia_config_store::get_own(COMPONENT, "recovery-log")
        .ok()
        .flatten()
        .unwrap_or_else(|| format!("{}/recovery.log", state_root()));
    if let Some(parent) = std::path::Path::new(&recovery_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&recovery_path)
    {
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(f, "{}\t{}\t{}", ts, "phoenix/restart", line);
    }
}

fn run_child_once(cmdline: &str) -> i32 {
    let output = Command::new("sh").arg("-lc").arg(cmdline).output();
    match output {
        Ok(out) if out.status.success() => 0,
        Ok(out) => {
            let code = out.status.code().unwrap_or(-1);
            let stderr = String::from_utf8_lossy(&out.stderr);
            append_trauma(&format!("child-exit={code} stderr={stderr}"));
            code
        }
        Err(e) => {
            append_trauma(&format!("child-exec-failed: {e}"));
            -1
        }
    }
}

fn main() {
    let env_mode = harmonia_config_store::get_own_or(COMPONENT, "env", "test")
        .unwrap_or_else(|_| "test".to_string());
    if env_mode.eq_ignore_ascii_case("prod") && !config_bool("allow-prod-genesis", false) {
        eprintln!(
            "[phoenix] refusing to start genesis in prod without allow-prod-genesis=1 in config-store"
        );
        std::process::exit(2);
    }

    let mut heartbeat_secs = 5_u64;
    if let Some(raw) = env::args().nth(1) {
        if let Ok(parsed) = raw.parse::<u64>() {
            heartbeat_secs = parsed.max(1);
        }
    }

    eprintln!(
        "[phoenix] supervisor online (env={}, heartbeat={}s)",
        env_mode, heartbeat_secs
    );

    if let Ok(child_cmd) = env::var("PHOENIX_CHILD_CMD") {
        let max_restarts = env::var("PHOENIX_MAX_RESTARTS")
            .ok()
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(3);
        for attempt in 0..=max_restarts {
            let rc = run_child_once(&child_cmd);
            if rc == 0 {
                eprintln!("[phoenix] child exited successfully.");
                return;
            }
            eprintln!(
                "[phoenix] child failed rc={} attempt={}/{}",
                rc,
                attempt + 1,
                max_restarts + 1
            );
            if attempt == max_restarts {
                eprintln!("[phoenix] max restarts exceeded.");
                std::process::exit(1);
            }
        }
    }

    loop {
        thread::sleep(Duration::from_secs(heartbeat_secs));
        eprintln!("[phoenix] heartbeat");
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn phoenix_test_harness_runs() {
        assert_eq!(super::config_bool("does-not-exist", true), true);
    }
}
