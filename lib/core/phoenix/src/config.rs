use serde::Deserialize;
use std::collections::HashMap;
use std::env;

use crate::msg::RestartPolicy;

const COMPONENT: &str = "phoenix-core";

// ── TOML structures ─────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct PhoenixToml {
    pub phoenix: Option<PhoenixSection>,
    #[serde(default)]
    pub subsystem: Vec<SubsystemToml>,
}

#[derive(Debug, Deserialize)]
pub struct PhoenixSection {
    pub health_port: Option<u16>,
    pub shutdown_timeout_secs: Option<u64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct SubsystemToml {
    pub name: String,
    pub command: String,
    #[serde(default)]
    pub core: bool,
    #[serde(default = "default_restart_policy")]
    pub restart_policy: String,
    #[serde(default = "default_max_restarts")]
    pub max_restarts: u32,
    #[serde(default = "default_backoff_base")]
    pub backoff_base_ms: u64,
    #[serde(default = "default_backoff_max")]
    pub backoff_max_ms: u64,
    #[serde(default)]
    pub startup_delay_ms: u64,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

fn default_restart_policy() -> String { "always".to_string() }
fn default_max_restarts() -> u32 { 3 }
fn default_backoff_base() -> u64 { 500 }
fn default_backoff_max() -> u64 { 60_000 }

// ── Resolved config ─────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct SubsystemConfig {
    pub name: String,
    pub command: String,
    pub core: bool,
    pub restart_policy: RestartPolicy,
    pub max_restarts: u32,
    pub backoff_base_ms: u64,
    pub backoff_max_ms: u64,
    pub startup_delay_ms: u64,
    pub env: HashMap<String, String>,
}

#[derive(Debug)]
pub struct PhoenixConfig {
    pub health_port: u16,
    pub shutdown_timeout_secs: u64,
    pub subsystems: Vec<SubsystemConfig>,
}

fn parse_restart_policy(s: &str) -> RestartPolicy {
    match s.to_ascii_lowercase().as_str() {
        "on_failure" | "onfailure" => RestartPolicy::OnFailure,
        "never" => RestartPolicy::Never,
        _ => RestartPolicy::Always,
    }
}

fn resolve_subsystem(raw: SubsystemToml) -> SubsystemConfig {
    let mut env = raw.env;
    for v in env.values_mut() {
        *v = interpolate_env(v);
    }
    SubsystemConfig {
        name: raw.name,
        command: interpolate_env(&raw.command),
        core: raw.core,
        restart_policy: parse_restart_policy(&raw.restart_policy),
        max_restarts: raw.max_restarts,
        backoff_base_ms: raw.backoff_base_ms,
        backoff_max_ms: raw.backoff_max_ms,
        startup_delay_ms: raw.startup_delay_ms,
        env,
    }
}

// ── Loader ──────────────────────────────────────────────────────────

pub fn load_or_legacy() -> Result<PhoenixConfig, String> {
    // 1. Try explicit config path from config-store or env
    let config_path = env::var("PHOENIX_CONFIG_PATH").ok().or_else(|| {
        harmonia_config_store::get_own(COMPONENT, "config-path")
            .ok()
            .flatten()
    });

    // 2. Fallback paths
    let candidates: Vec<String> = if let Some(p) = config_path {
        vec![p]
    } else {
        let state = state_root();
        vec![
            format!("{}/phoenix.toml", state),
            "phoenix.toml".to_string(),
        ]
    };

    for path in &candidates {
        if let Ok(contents) = std::fs::read_to_string(path) {
            let parsed: PhoenixToml =
                toml::from_str(&contents).map_err(|e| format!("bad TOML in {path}: {e}"))?;
            let health_port = parsed
                .phoenix
                .as_ref()
                .and_then(|p| p.health_port)
                .unwrap_or(9100);
            let shutdown_timeout_secs = parsed
                .phoenix
                .as_ref()
                .and_then(|p| p.shutdown_timeout_secs)
                .unwrap_or(30);
            let subsystems = parsed
                .subsystem
                .into_iter()
                .map(resolve_subsystem)
                .collect();
            return Ok(PhoenixConfig {
                health_port,
                shutdown_timeout_secs,
                subsystems,
            });
        }
    }

    // 3. Legacy fallback: synthesize from config-store keys
    legacy_fallback()
}

fn legacy_fallback() -> Result<PhoenixConfig, String> {
    let child_cmd = harmonia_config_store::get_own(COMPONENT, "child-cmd")
        .ok()
        .flatten();

    match child_cmd {
        Some(cmd) => {
            let max_restarts = harmonia_config_store::get_own(COMPONENT, "max-restarts")
                .ok()
                .flatten()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(3);
            Ok(PhoenixConfig {
                health_port: 9100,
                shutdown_timeout_secs: 30,
                subsystems: vec![SubsystemConfig {
                    name: "child".to_string(),
                    command: interpolate_env(&cmd),
                    core: true,
                    restart_policy: RestartPolicy::Always,
                    max_restarts,
                    backoff_base_ms: default_backoff_base(),
                    backoff_max_ms: default_backoff_max(),
                    startup_delay_ms: 0,
                    env: HashMap::new(),
                }],
            })
        }
        None => Err("no phoenix.toml found and no child-cmd configured".to_string()),
    }
}

fn interpolate_env(s: &str) -> String {
    let mut result = s.to_string();
    while let Some(start) = result.find("${") {
        let rest = &result[start + 2..];
        if let Some(end) = rest.find('}') {
            let var_name = &rest[..end];
            let value = env::var(var_name).unwrap_or_default();
            result = format!("{}{}{}", &result[..start], value, &rest[end + 1..]);
        } else {
            break;
        }
    }
    result
}

fn state_root() -> String {
    let default = env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    harmonia_config_store::get_config_or(COMPONENT, "global", "state-root", &default)
        .unwrap_or_else(|_| default)
}
