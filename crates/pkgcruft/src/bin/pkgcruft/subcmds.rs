use std::process::ExitCode;

use pkgcraft::config::Config;

mod scan;
mod show;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Scan targets for QA issues
    Scan(scan::Command),
    /// Show various information
    Show(show::Command),
}

impl Subcommand {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Scan(cmd) => cmd.run(config),
            Show(cmd) => cmd.run(),
        }
    }
}
