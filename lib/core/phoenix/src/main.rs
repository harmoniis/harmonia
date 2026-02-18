use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::process::Command;
use std::thread;
use std::time::Duration;

fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
        .unwrap_or(default)
}

fn append_trauma(line: &str) {
    let trauma_path = env::var("PHOENIX_TRAUMA_LOG")
        .unwrap_or_else(|_| "/tmp/harmonia/trauma.log".to_string());
    if let Some(parent) = std::path::Path::new(&trauma_path).parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&trauma_path) {
        let _ = writeln!(f, "{line}");
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
    let env_mode = env::var("HARMONIA_ENV").unwrap_or_else(|_| "test".to_string());
    if env_mode.eq_ignore_ascii_case("prod") && !env_bool("HARMONIA_ALLOW_PROD_GENESIS", false) {
        eprintln!("[phoenix] refusing to start genesis in prod without HARMONIA_ALLOW_PROD_GENESIS=1");
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
            eprintln!("[phoenix] child failed rc={} attempt={}/{}", rc, attempt + 1, max_restarts + 1);
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
        assert_eq!(super::env_bool("DOES_NOT_EXIST", true), true);
    }
}
