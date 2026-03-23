mod broker;
mod frontend_pairing;
mod node_link;
mod node_rpc;
#[cfg(unix)]
mod node_service;
#[cfg(not(unix))]
mod node_service {
    pub fn run_foreground() -> Result<(), Box<dyn std::error::Error>> {
        Err("node-service currently requires Unix local socket support on this platform".into())
    }

    pub fn ensure_background(
        _node: &crate::paths::NodeIdentity,
    ) -> Result<(), Box<dyn std::error::Error>> {
        Err("node-service currently requires Unix local socket support on this platform".into())
    }
}
mod pairing;
mod remote;
#[cfg(unix)]
mod session;
#[cfg(not(unix))]
mod session {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        Err("interactive local sessions currently require Unix domain sockets and are unavailable on this platform".into())
    }
}
mod draft_store;
mod edit_buffer;
mod input_history;
mod menus;
#[cfg(unix)]
mod modules;
mod paths;
mod service;
mod setup;
mod start;
mod stop;
mod tailscale_local;
mod uninstall;
mod upgrade;

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "harmonia",
    about = "Harmonia — self-improving Common Lisp + Rust agent",
    version = VERSION,
    after_help = "Run `harmonia` with no arguments to open the interactive TUI session."
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive setup wizard — configure API keys, frontends, and workspace
    Setup {
        /// Configure model seed policy only
        #[arg(long)]
        seeds: bool,
        /// Apply config from a JSON file (headless/automated provisioning)
        #[arg(long = "headless-config")]
        headless_config: Option<String>,
    },
    /// Start the Harmonia daemon
    Start {
        /// Environment (test, dev, prod)
        #[arg(short, long, default_value = "dev")]
        env: String,
        /// Run in foreground (don't daemonize)
        #[arg(long)]
        foreground: bool,
        /// Enable debug logging (verbose output)
        #[arg(long)]
        debug: bool,
    },
    /// Stop the Harmonia daemon
    Stop,
    /// Restart the Harmonia daemon (stop + start)
    Restart {
        /// Environment (test, dev, prod)
        #[arg(short, long, default_value = "dev")]
        env: String,
        /// Enable debug logging (verbose output)
        #[arg(long)]
        debug: bool,
    },
    /// Run the embedded MQTT broker/runtime helper
    #[command(hide = true)]
    Broker,
    /// Show daemon status
    Status,
    /// Pair remote session clients with agent nodes
    Pairing {
        #[command(subcommand)]
        action: PairingAction,
    },
    /// Call typed RPC operations on the paired remote node
    Remote {
        #[command(subcommand)]
        action: RemoteAction,
    },
    /// Install/uninstall system service for auto-start on boot
    Service {
        #[command(subcommand)]
        action: ServiceAction,
    },
    /// Uninstall Harmonia (preserves user data). Use evolution-export/import for backups.
    Uninstall {
        #[command(subcommand)]
        action: Option<uninstall::UninstallAction>,
    },
    /// Upgrade to the latest release (preserves evolved state and wallet data)
    Upgrade,
    /// Run the local tailscale node-service (session relay + RPC)
    #[command(hide = true)]
    NodeService,
    /// Manage runtime modules (list, load, unload, reload)
    Modules {
        #[command(subcommand)]
        action: Option<ModulesAction>,
    },
    /// Show version and system info
    Version,
}

#[derive(Subcommand)]
enum ModulesAction {
    /// List all modules and their status
    List,
    /// Load a module by name
    Load { name: String },
    /// Unload a module by name
    Unload { name: String },
    /// Reload a module (unload + load)
    Reload { name: String },
}

#[derive(Subcommand)]
enum ServiceAction {
    /// Install system service (launchd/systemd/rc.d)
    Install,
    /// Remove system service
    Uninstall,
}

#[derive(Subcommand)]
enum PairingAction {
    /// Print a one-line pairing code to paste on a remote client
    Invite,
    /// Show the current saved pairing for this node
    Show,
}

#[derive(Subcommand)]
pub enum RemoteAction {
    /// Show the paired node's capabilities and granted operations
    Capabilities,
    /// Remote filesystem operations
    Fs {
        #[command(subcommand)]
        action: RemoteFsAction,
    },
    /// Run a command on the paired node
    Shell {
        program: String,
        args: Vec<String>,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long, default_value_t = 30_000)]
        timeout_ms: u64,
    },
    /// Remote tmux operations
    Tmux {
        #[command(subcommand)]
        action: RemoteTmuxAction,
    },
    /// Remote wallet and vault operations
    Wallet {
        #[command(subcommand)]
        action: RemoteWalletAction,
    },
}

#[derive(Subcommand)]
pub enum RemoteFsAction {
    /// List a directory on the paired node
    List {
        path: String,
        #[arg(long)]
        hidden: bool,
        #[arg(long, default_value_t = 256)]
        max_entries: u32,
    },
    /// Read a text file on the paired node
    Read {
        path: String,
        #[arg(long, default_value_t = 65_536)]
        max_bytes: u64,
    },
}

#[derive(Subcommand)]
pub enum RemoteTmuxAction {
    /// List tmux sessions on the paired node
    List,
    /// Spawn a tmux session on the paired node
    Spawn {
        session: String,
        #[arg(long)]
        cwd: Option<String>,
        #[arg(long)]
        command: Option<String>,
        args: Vec<String>,
    },
    /// Capture a tmux pane from the paired node
    Capture {
        session: String,
        #[arg(long, default_value_t = 200)]
        history: u32,
    },
    /// Send a line of input to a tmux session
    Send { session: String, input: String },
    /// Send a special key to a tmux session
    Key { session: String, key: String },
}

#[derive(Subcommand)]
pub enum RemoteWalletAction {
    /// Show wallet/vault status on the paired node
    Status,
    /// List available vault symbols on the paired node
    Symbols,
    /// Check whether a vault symbol exists on the paired node
    Has { symbol: String },
    /// Set a vault symbol on the paired node
    Set { symbol: String, value: String },
}

/// Reset terminal to sane cooked mode if a previous process left it in raw mode.
/// This fixes the "staircase" output pattern where \n doesn't return to column 0.
/// Must use raw libc calls because crossterm::disable_raw_mode is a no-op
/// if enable_raw_mode wasn't called in THIS process.
#[cfg(unix)]
fn reset_terminal_if_needed() {
    use std::os::unix::io::AsRawFd;
    // Only reset if stderr is a real terminal (not piped)
    let fd = std::io::stderr().as_raw_fd();
    if unsafe { libc::isatty(fd) } != 1 {
        return;
    }
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
        return;
    }
    // Check if OPOST (output post-processing) is off — this is the raw mode symptom.
    // In cooked mode, OPOST is on which maps \n to \r\n.
    if termios.c_oflag & libc::OPOST == 0 {
        // Terminal is in raw mode. Force it back to cooked.
        termios.c_oflag |= libc::OPOST;
        termios.c_lflag |= libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN;
        termios.c_iflag |= libc::ICRNL | libc::IXON;
        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
    }
}

fn main() {
    // Reset terminal state in case a previous session crashed with raw mode on.
    // crossterm::disable_raw_mode() is a no-op if enable wasn't called in THIS process,
    // so we use tcsetattr directly to force sane settings on the actual terminal.
    #[cfg(unix)]
    reset_terminal_if_needed();

    let cli = Cli::parse();

    match cli.command {
        // No subcommand = open TUI session (connect to running daemon)
        None => {
            if let Err(e) = session::run() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Setup {
            seeds,
            headless_config,
        }) => {
            let result = if let Some(config_path) = headless_config {
                setup::run_headless(&config_path)
            } else if seeds {
                setup::run_seeds_only()
            } else {
                setup::run()
            };
            if let Err(e) = result {
                eprintln!("Setup failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Start {
            env,
            foreground,
            debug,
        }) => {
            if debug {
                std::env::set_var("HARMONIA_LOG_LEVEL", "debug");
                let _ = paths::set_config_value("global", "log-level", "debug");
            }
            if let Err(e) = start::run(&env, foreground) {
                eprintln!("Start failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Stop) => {
            if let Err(e) = stop::run() {
                eprintln!("Stop failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Restart { env, debug }) => {
            if debug {
                std::env::set_var("HARMONIA_LOG_LEVEL", "debug");
                let _ = paths::set_config_value("global", "log-level", "debug");
            }
            let _ = stop::run(); // ignore error if not running
            std::thread::sleep(std::time::Duration::from_secs(1));
            if let Err(e) = start::run(&env, false) {
                eprintln!("Restart failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Broker) => {
            if let Err(e) = broker::run() {
                eprintln!("Broker failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Status) => {
            if let Err(e) = status() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Pairing { action }) => {
            let node = match paths::current_node_identity() {
                Ok(node) => node,
                Err(e) => {
                    eprintln!("Pairing failed: {}", e);
                    std::process::exit(1);
                }
            };
            let result = match action {
                PairingAction::Invite => pairing::print_invite(&node),
                PairingAction::Show => pairing::print_pairing(&node),
            };
            if let Err(e) = result {
                eprintln!("Pairing failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Remote { action }) => {
            if let Err(e) = remote::run(&action) {
                eprintln!("Remote call failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Service { action }) => {
            let result = match action {
                ServiceAction::Install => service::install(),
                ServiceAction::Uninstall => service::uninstall(),
            };
            if let Err(e) = result {
                eprintln!("Service failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Uninstall { action }) => {
            if let Err(e) = uninstall::run(action) {
                eprintln!("Uninstall failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Upgrade) => {
            if let Err(e) = upgrade::run() {
                eprintln!("Upgrade failed: {}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::NodeService) => {
            if let Err(e) = node_service::run_foreground() {
                eprintln!("Node service failed: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(unix)]
        Some(Commands::Modules { action }) => {
            let result = match action {
                None | Some(ModulesAction::List) => modules::list(),
                Some(ModulesAction::Load { name }) => modules::load(&name),
                Some(ModulesAction::Unload { name }) => modules::unload(&name),
                Some(ModulesAction::Reload { name }) => modules::reload(&name),
            };
            if let Err(e) = result {
                eprintln!("Modules command failed: {}", e);
                std::process::exit(1);
            }
        }
        #[cfg(not(unix))]
        Some(Commands::Modules { .. }) => {
            eprintln!("Module management requires Unix domain sockets");
            std::process::exit(1);
        }
        Some(Commands::Version) => {
            println!("harmonia {}", VERSION);
            println!("runtime: SBCL (Steel Bank Common Lisp)");
            println!("tools:   Rust rlib (compiled into harmonia-runtime)");
            if check_sbcl() {
                println!("sbcl:    installed");
            } else {
                println!("sbcl:    NOT FOUND — run `harmonia setup`");
            }
        }
    }
}

fn check_sbcl() -> bool {
    std::process::Command::new("sbcl")
        .arg("--version")
        .output()
        .is_ok()
}

fn status() -> Result<(), Box<dyn std::error::Error>> {
    let pid_path = paths::pid_path()?;
    let node_service_pid_path = paths::node_service_pid_path()?;
    let sock_path = paths::socket_path()?;
    let log_path = paths::log_path()?;

    if !pid_path.exists() {
        if node_service_pid_path.exists() {
            println!("Harmonia daemon is not running.");
            if let Ok(pid_str) = std::fs::read_to_string(&node_service_pid_path) {
                let log = paths::node_service_log_path()?;
                println!("Node service is running (PID {})", pid_str.trim());
                println!("  log: {}", log.display());
            }
            return Ok(());
        }
        println!("Harmonia is not running.");
        return Ok(());
    }

    let pid_str = std::fs::read_to_string(&pid_path)?;
    let pid: i32 = pid_str.trim().parse().map_err(|_| "invalid PID file")?;

    #[cfg(unix)]
    {
        let alive = unsafe { libc::kill(pid, 0) } == 0;
        if !alive {
            println!("Harmonia is not running (stale PID file for {}).", pid);
            let _ = std::fs::remove_file(&pid_path);
            return Ok(());
        }

        // ── Header ──────────────────────────────────────────────────
        println!();
        println!(
            "  {} Harmonia is {} (PID {})",
            console::style("●").green().bold(),
            console::style("running").green().bold(),
            pid
        );
        println!();

        // ── Subsystems (from Phoenix health) ────────────────────────
        println!("  {}", console::style("Subsystems").cyan().bold());
        println!(
            "  {}",
            console::style("────────────────────────────────────────").dim()
        );
        match query_phoenix_health() {
            Ok(json) => {
                if let Ok(health) = serde_json::from_str::<serde_json::Value>(&json) {
                    // Uptime
                    if let Some(uptime) = health.get("uptime_secs").and_then(|v| v.as_u64()) {
                        let hours = uptime / 3600;
                        let mins = (uptime % 3600) / 60;
                        let secs = uptime % 60;
                        println!("  uptime:           {}h {}m {}s", hours, mins, secs);
                    }
                    // Mode
                    if let Some(mode) = health.pointer("/mode/mode").and_then(|v| v.as_str()) {
                        let styled = match mode {
                            "full" => console::style(mode).green().to_string(),
                            "starting" => console::style(mode).yellow().to_string(),
                            _ => mode.to_string(),
                        };
                        println!("  mode:             {}", styled);
                    }
                    // Subsystem status
                    if let Some(subs) = health.get("subsystems").and_then(|v| v.as_object()) {
                        for (name, info) in subs {
                            let status = info
                                .get("status")
                                .and_then(|v| v.as_str())
                                .unwrap_or("unknown");
                            let styled = match status {
                                "running" => console::style(status).green().to_string(),
                                "backoff" | "starting" => {
                                    console::style(status).yellow().to_string()
                                }
                                "stopped" | "crashed" => console::style(status).red().to_string(),
                                _ => status.to_string(),
                            };
                            let mut detail = String::new();
                            if let Some(attempt) = info.get("attempt").and_then(|v| v.as_u64()) {
                                detail = format!(" (attempt {}/10)", attempt);
                            }
                            println!("  {:<18} {}{}", name, styled, detail);
                        }
                    }
                } else {
                    println!("  health:           {}", json);
                }
            }
            Err(_) => {
                println!(
                    "  health:           {}",
                    console::style("unavailable").red()
                );
            }
        }

        // ── Modules (from runtime IPC) ──────────────────────────────
        #[cfg(unix)]
        {
            if let Ok(module_output) = query_runtime_modules() {
                println!();
                println!("  {}", console::style("Modules").cyan().bold());
                println!(
                    "  {}",
                    console::style("────────────────────────────────────────").dim()
                );

                let mut loaded = Vec::new();
                let mut errors = Vec::new();
                let mut unloaded = Vec::new();

                for m in &module_output {
                    match m.1.as_str() {
                        "loaded" => loaded.push(&m.0),
                        "unloaded" => unloaded.push(&m.0),
                        _ => errors.push(m),
                    }
                }

                // Loaded
                if !loaded.is_empty() {
                    let names: Vec<&str> = loaded.iter().map(|s| s.as_str()).collect();
                    println!(
                        "  {} loaded:      {}",
                        console::style(loaded.len()).green().bold(),
                        console::style(names.join(", ")).dim()
                    );
                }
                // Errors (unconfigured)
                if !errors.is_empty() {
                    println!(
                        "  {} unconfigured:",
                        console::style(errors.len()).yellow().bold(),
                    );
                    for (name, _, needs) in &errors {
                        if !needs.is_empty() {
                            let clean = needs.replace("\\\"", "\"").replace("\\\\", "\\");
                            println!("    {:<18} needs: {}", name, console::style(&clean).dim());
                        } else {
                            println!("    {}", name);
                        }
                    }
                }
            }
        }

        // ── Paths ───────────────────────────────────────────────────
        println!();
        println!("  {}", console::style("Paths").cyan().bold());
        println!(
            "  {}",
            console::style("────────────────────────────────────────").dim()
        );
        if sock_path.exists() {
            println!(
                "  socket:           {}",
                console::style(sock_path.display()).dim()
            );
        }
        println!(
            "  log:              {}",
            console::style(log_path.display()).dim()
        );
        if node_service_pid_path.exists() {
            if let Ok(ns_pid) = std::fs::read_to_string(&node_service_pid_path) {
                println!("  node-service:     PID {}", ns_pid.trim());
            }
        }

        println!();
        println!(
            "  {}       to open session",
            console::style("harmonia").cyan().bold()
        );
        println!(
            "  {}  to stop",
            console::style("harmonia stop").cyan().bold()
        );
        println!(
            "  {} to manage modules",
            console::style("harmonia modules").cyan().bold()
        );
        println!();
    }

    Ok(())
}

/// Query runtime IPC for module list. Returns Vec of (name, status, needs).
#[cfg(unix)]
fn query_runtime_modules() -> Result<Vec<(String, String, String)>, Box<dyn std::error::Error>> {
    use std::io::{Read, Write};
    use std::os::unix::net::UnixStream;

    let data_dir = paths::data_dir()?;
    if std::env::var_os("HARMONIA_STATE_ROOT").is_none() {
        std::env::set_var("HARMONIA_STATE_ROOT", data_dir.to_string_lossy().as_ref());
    }
    let _ = harmonia_config_store::init_v2();
    let default = std::env::temp_dir()
        .join("harmonia")
        .to_string_lossy()
        .to_string();
    let state_root =
        harmonia_config_store::get_config_or("harmonia-runtime", "global", "state-root", &default)
            .unwrap_or(default);
    let sock_path = std::path::PathBuf::from(state_root).join("runtime.sock");
    if !sock_path.exists() {
        return Err("runtime socket not found".into());
    }

    let mut stream = UnixStream::connect(&sock_path)?;
    stream.set_read_timeout(Some(std::time::Duration::from_secs(3)))?;
    let msg = b"(:modules :op \"list\")";
    let len = (msg.len() as u32).to_be_bytes();
    stream.write_all(&len)?;
    stream.write_all(msg)?;
    stream.flush()?;

    let mut hdr = [0u8; 4];
    stream.read_exact(&mut hdr)?;
    let rlen = u32::from_be_bytes(hdr) as usize;
    let mut buf = vec![0u8; rlen];
    stream.read_exact(&mut buf)?;
    let sexp = String::from_utf8_lossy(&buf).to_string();

    // Simple parse: extract (:name "X" :status Y ...) entries
    let mut result = Vec::new();
    let chars: Vec<char> = sexp.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if i + 5 < chars.len() && chars[i] == '(' && chars[i + 1] == ':' && chars[i + 2] == 'n' {
            let start = i;
            let mut depth = 0;
            let mut end = i;
            for j in start..chars.len() {
                if chars[j] == '(' {
                    depth += 1;
                } else if chars[j] == ')' {
                    depth -= 1;
                    if depth == 0 {
                        end = j + 1;
                        break;
                    }
                }
            }
            let entry: String = chars[start..end].iter().collect();
            if let Some(name) = extract_sexp_quoted(&entry, ":name") {
                let status = extract_sexp_unquoted(&entry, ":status").unwrap_or_default();
                let needs = extract_sexp_quoted(&entry, ":needs").unwrap_or_default();
                result.push((name, status, needs));
            }
            i = end;
        } else {
            i += 1;
        }
    }
    Ok(result)
}

#[cfg(unix)]
fn extract_sexp_quoted(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    if !after.starts_with('"') {
        return None;
    }
    let bytes = after[1..].as_bytes();
    let mut end = 0;
    while end < bytes.len() {
        if bytes[end] == b'"' {
            return Some(
                after[1..1 + end]
                    .replace("\\\"", "\"")
                    .replace("\\\\", "\\"),
            );
        }
        if bytes[end] == b'\\' {
            end += 1;
        }
        end += 1;
    }
    None
}

#[cfg(unix)]
fn extract_sexp_unquoted(sexp: &str, key: &str) -> Option<String> {
    let idx = sexp.find(key)?;
    let after = sexp[idx + key.len()..].trim_start();
    let val: String = after
        .chars()
        .take_while(|c| !c.is_whitespace() && *c != ')' && *c != '"')
        .collect();
    if val.is_empty() {
        None
    } else {
        Some(val)
    }
}

fn query_phoenix_health() -> Result<String, Box<dyn std::error::Error>> {
    let health_port = 9100u16; // convention, matches phoenix.toml default
    let url = format!("http://127.0.0.1:{}/health", health_port);
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .call()?;
    Ok(resp.into_string()?)
}
