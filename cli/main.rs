mod broker;
#[cfg(unix)]
mod chat;
#[cfg(not(unix))]
mod chat {
    pub fn run() -> Result<(), Box<dyn std::error::Error>> {
        Err("interactive local chat currently requires Unix domain sockets and is unavailable on this platform".into())
    }
}
mod menus;
mod paths;
mod service;
mod setup;
mod start;
mod stop;
mod uninstall;
mod upgrade;

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "harmonia",
    about = "Harmonia — self-improving Common Lisp + Rust agent",
    version = VERSION,
    after_help = "Run `harmonia` with no arguments to open the interactive TUI."
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

fn main() {
    let cli = Cli::parse();

    match cli.command {
        // No subcommand = open TUI (connect to running daemon)
        None => {
            if let Err(e) = chat::run() {
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
        Some(Commands::Version) => {
            println!("harmonia {}", VERSION);
            println!("runtime: SBCL (Steel Bank Common Lisp)");
            println!("tools:   Rust cdylib (.so/.dylib)");
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
    let sock_path = paths::socket_path()?;
    let log_path = paths::log_path()?;
    let broker_log_path = paths::broker_log_path()?;

    if !pid_path.exists() {
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
                "  {}       to open TUI",
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
