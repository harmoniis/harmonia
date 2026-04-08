//! Clap struct definitions for the CLI argument parser.

use clap::{Parser, Subcommand};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Parser)]
#[command(
    name = "harmonia",
    about = "Harmonia — self-improving Common Lisp + Rust agent",
    version = VERSION,
    after_help = "Run `harmonia` with no arguments to open the interactive TUI session."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
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
        action: Option<super::uninstall::UninstallAction>,
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
pub enum ModulesAction {
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
pub enum ServiceAction {
    /// Install system service (launchd/systemd/rc.d)
    Install,
    /// Remove system service
    Uninstall,
}

#[derive(Subcommand)]
pub enum PairingAction {
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
