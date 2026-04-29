use std::env;
use std::fs::{self, OpenOptions};
use std::io::Write;

const COMPONENT: &str = "phoenix-core";

pub fn state_root() -> String {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or_else(|_| default)
}

pub fn chronicle_record(
    event_type: &str,
    exit_code: Option<i32>,
    attempt: Option<i32>,
    max_attempts: Option<i32>,
    detail: Option<&str>,
) {
    let _ = harmonia_chronicle::phoenix::record(
        event_type,
        exit_code,
        attempt,
        max_attempts,
        None,
        detail,
    );
}

/// Record crash to Ouroboros crash ledger for self-healing feedback.
pub fn ouroboros_record(component: &str, detail: &str) {
    let mut state = harmonia_ouroboros::OuroborosState::new();
    let _ = harmonia_ouroboros::dispatch(
        &mut state,
        &format!(
            "(:op \"record-crash\" :component-name \"{}\" :detail \"{}\")",
            component.replace('"', "\\\""),
            detail.replace('"', "\\\""),
        ),
    );
}

pub fn append_trauma(line: &str) {
    let default_trauma = format!("{}/trauma.log", state_root());
    let trauma_path = harmonia_config_store::get_own_or(COMPONENT, "trauma-log", &default_trauma)
        .unwrap_or(default_trauma);
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

    let recovery_path = harmonia_config_store::get_config(COMPONENT, "global", "recovery-log")
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
