mod setup;
mod start;
mod uninstall;
mod upgrade;

use clap::{Parser, Subcommand};

const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "harmonia",
    about = "Harmonia — self-improving Common Lisp + Rust agent",
    version = VERSION,
    after_help = "Get started: harmonia setup"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Interactive setup wizard — configure API keys, frontends, and workspace
    Setup,
    /// Start the Harmonia agent
    Start {
        /// Environment (test, dev, prod)
        #[arg(short, long, default_value = "dev")]
        env: String,
    },
    /// Uninstall Harmonia (preserves wallet data)
    Uninstall,
    /// Upgrade to the latest release (preserves evolved state and wallet data)
    Upgrade,
    /// Show version and system info
    Version,
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Setup => {
            if let Err(e) = setup::run() {
                eprintln!("Setup failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Start { env } => {
            if let Err(e) = start::run(&env) {
                eprintln!("Start failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Uninstall => {
            if let Err(e) = uninstall::run() {
                eprintln!("Uninstall failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Upgrade => {
            if let Err(e) = upgrade::run() {
                eprintln!("Upgrade failed: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Version => {
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
