//! Unified frontend setup and pairing flow for the interactive Harmonia CLI.

mod catalog;
mod discovery;
mod pairing_flow;

use std::io::Stdout;

const BOLD_CYAN: &str = "\x1b[1;36m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const RED: &str = "\x1b[31m";
const RESET: &str = "\x1b[0m";

pub enum PairingTarget {
    Local {
        node_label: String,
    },
    Remote {
        node: crate::paths::NodeIdentity,
        pairing: crate::pairing::PairingRecord,
    },
}

impl PairingTarget {
    fn node_label(&self) -> &str {
        match self {
            PairingTarget::Local { node_label } => node_label,
            PairingTarget::Remote { pairing, .. } => &pairing.remote_label,
        }
    }
}

pub fn run_pairing_menu(
    stdout: &mut Stdout,
    node: &crate::paths::NodeIdentity,
) -> Result<(), Box<dyn std::error::Error>> {
    pairing_flow::run_pairing_menu(stdout, node)
}
