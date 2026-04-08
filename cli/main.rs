mod broker;
mod cli_args;
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
#[cfg(unix)]
mod session_flows;
#[cfg(unix)]
mod session_ipc;
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
mod status;
mod status_modules;
mod stop;
mod tailscale_local;
mod uninstall;
mod upgrade;

use clap::Parser;
use cli_args::*;

/// Reset terminal to sane cooked mode if a previous process left it in raw mode.
#[cfg(unix)]
fn reset_terminal_if_needed() {
    use std::os::unix::io::AsRawFd;
    let fd = std::io::stderr().as_raw_fd();
    if unsafe { libc::isatty(fd) } != 1 {
        return;
    }
    let mut termios: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut termios) } != 0 {
        return;
    }
    if termios.c_oflag & libc::OPOST == 0 {
        termios.c_oflag |= libc::OPOST;
        termios.c_lflag |= libc::ECHO | libc::ICANON | libc::ISIG | libc::IEXTEN;
        termios.c_iflag |= libc::ICRNL | libc::IXON;
        unsafe { libc::tcsetattr(fd, libc::TCSANOW, &termios) };
    }
}

fn main() {
    #[cfg(unix)]
    reset_terminal_if_needed();

    let cli = Cli::parse();

    let result = match cli.command {
        None => session::run(),
        Some(Commands::Setup {
            seeds,
            headless_config,
        }) => {
            if let Some(config_path) = headless_config {
                setup::run_headless(&config_path)
            } else if seeds {
                setup::run_seeds_only()
            } else {
                setup::run()
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
            start::run(&env, foreground)
        }
        Some(Commands::Stop) => stop::run(),
        Some(Commands::Restart { env, debug }) => {
            if debug {
                std::env::set_var("HARMONIA_LOG_LEVEL", "debug");
                let _ = paths::set_config_value("global", "log-level", "debug");
            }
            let _ = stop::run();
            std::thread::sleep(std::time::Duration::from_secs(1));
            start::run(&env, false)
        }
        Some(Commands::Broker) => broker::run(),
        Some(Commands::Status) => status::status(),
        Some(Commands::Pairing { action }) => {
            let node = match paths::current_node_identity() {
                Ok(node) => node,
                Err(e) => {
                    eprintln!("Pairing failed: {}", e);
                    std::process::exit(1);
                }
            };
            match action {
                PairingAction::Invite => pairing::print_invite(&node),
                PairingAction::Show => pairing::print_pairing(&node),
            }
        }
        Some(Commands::Remote { action }) => remote::run(&action),
        Some(Commands::Service { action }) => match action {
            ServiceAction::Install => service::install(),
            ServiceAction::Uninstall => service::uninstall(),
        },
        Some(Commands::Uninstall { action }) => uninstall::run(action),
        Some(Commands::Upgrade) => upgrade::run(),
        Some(Commands::NodeService) => node_service::run_foreground(),
        #[cfg(unix)]
        Some(Commands::Modules { action }) => match action {
            None | Some(ModulesAction::List) => modules::list(),
            Some(ModulesAction::Load { name }) => modules::load(&name),
            Some(ModulesAction::Unload { name }) => modules::unload(&name),
            Some(ModulesAction::Reload { name }) => modules::reload(&name),
        },
        #[cfg(not(unix))]
        Some(Commands::Modules { .. }) => {
            eprintln!("Module management requires Unix domain sockets");
            std::process::exit(1);
        }
        Some(Commands::Version) => {
            println!("harmonia {} ({} {})", VERSION, cli_args::BUILD_HASH, cli_args::BUILD_DATE);
            println!("build:   {} ({})", cli_args::BUILD_HASH, cli_args::BUILD_DATE);
            println!("runtime: SBCL (Steel Bank Common Lisp)");
            println!("tools:   Rust rlib (compiled into harmonia-runtime)");
            if check_sbcl() {
                println!("sbcl:    installed");
            } else {
                println!("sbcl:    NOT FOUND — run `harmonia setup`");
            }
            println!("\nRun `harmonia upgrade` to check for updates.");
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn check_sbcl() -> bool {
    std::process::Command::new("sbcl")
        .arg("--version")
        .output()
        .is_ok()
}
