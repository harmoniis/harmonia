//! Uninstall flow: evolution export/import and system cleanup.

mod cleanup;
mod evolution;
mod helpers;

/// Subcommands for `harmonia uninstall`
#[derive(clap::Subcommand, Clone)]
pub enum UninstallAction {
    /// Export evolution state (versions, snapshots, config) to a portable archive
    #[command(name = "evolution-export")]
    EvolutionExport {
        /// Output path for the evolution archive (.tar.gz)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// Import a previously exported evolution archive into a fresh installation
    #[command(name = "evolution-import")]
    EvolutionImport {
        /// Path to the evolution archive (.tar.gz) to import
        path: String,
        /// Merge with existing evolution state instead of replacing
        #[arg(long)]
        merge: bool,
    },
}

pub fn run(action: Option<UninstallAction>) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        Some(UninstallAction::EvolutionExport { output }) => evolution::run_evolution_export(output),
        Some(UninstallAction::EvolutionImport { path, merge }) => {
            evolution::run_evolution_import(&path, merge)
        }
        None => cleanup::run_uninstall(),
    }
}
