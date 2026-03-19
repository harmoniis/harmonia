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
mod menus;
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
    /// Show version and system info
    Version,
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // No subcommand = open TUI session (connect to running daemon)
        None => {
            if let Err(e) = session::run() {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Some(Commands::Setup { seeds }) => {
            let result = if seeds {
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
    let broker_pid_path = paths::broker_pid_path()?;
    let node_service_pid_path = paths::node_service_pid_path()?;
    let sock_path = paths::socket_path()?;
    let log_path = paths::log_path()?;
    let broker_log_path = paths::broker_log_path()?;
    let node_service_log_path = paths::node_service_log_path()?;

    if !pid_path.exists() {
        if node_service_pid_path.exists() {
            println!("Harmonia daemon is not running.");
            if let Ok(pid_str) = std::fs::read_to_string(&node_service_pid_path) {
                println!("Node service is running (PID {})", pid_str.trim());
                println!("  log:    {}", node_service_log_path.display());
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
        if alive {
            println!("Harmonia is running (PID {})", pid);
            if sock_path.exists() {
                println!("  socket: {}", sock_path.display());
            }
            println!("  log:    {}", log_path.display());
            // Query Phoenix health endpoint
            match query_phoenix_health() {
                Ok(json) => {
                    println!("  health: {}", json);
                }
                Err(_) => {
                    println!("  health: unavailable (Phoenix health endpoint not responding)");
                }
            }
            if node_service_pid_path.exists() {
                if let Ok(pid_str) = std::fs::read_to_string(&node_service_pid_path) {
                    println!("  node-service pid: {}", pid_str.trim());
                    println!("  node-service log: {}", node_service_log_path.display());
                }
            }
            if broker_pid_path.exists() {
                if let Ok(pid_str) = std::fs::read_to_string(&broker_pid_path) {
                    println!(
                        "  broker: {} (PID {})",
                        broker_log_path.display(),
                        pid_str.trim()
                    );
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
        } else {
            println!("Harmonia is not running (stale PID file for {}).", pid);
            let _ = std::fs::remove_file(&pid_path);
        }
    }

    Ok(())
}

fn query_phoenix_health() -> Result<String, Box<dyn std::error::Error>> {
    let health_port = 9100u16; // convention, matches phoenix.toml default
    let url = format!("http://127.0.0.1:{}/health", health_port);
    let resp = ureq::get(&url)
        .timeout(std::time::Duration::from_secs(3))
        .call()?;
    Ok(resp.into_string()?)
}
