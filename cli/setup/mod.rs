//! Interactive setup wizard — configure API keys, frontends, and workspace.

mod gateway_config;
mod headless;
pub(crate) mod helpers;
mod optional;
mod providers;
mod providers_config;
mod seed_policy;
mod wizard;

use gateway_config::resolve_configured_modules;

const BANNER: &str = r#"
  _   _                                  _
 | | | | __ _ _ __ _ __ ___   ___  _ __ (_) __ _
 | |_| |/ _` | '__| '_ ` _ \ / _ \| '_ \| |/ _` |
 |  _  | (_| | |  | | | | | | (_) | | | | | (_| |
 |_| |_|\__,_|_|  |_| |_| |_|\___/|_| |_|_|\__,_|
"#;

pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    wizard::run()
}

pub fn run_seeds_only() -> Result<(), Box<dyn std::error::Error>> {
    wizard::run_seeds_only()
}

pub fn run_headless(config_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    headless::run_headless(config_path)
}
