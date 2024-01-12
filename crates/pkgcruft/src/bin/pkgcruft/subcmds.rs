use std::process::ExitCode;

use pkgcraft::config::Config;

mod replay;
mod scan;
mod show;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Replay reports
    Replay(replay::Command),
    /// Scan targets for QA issues
    Scan(scan::Command),
    /// Show various information
    Show(show::Command),
}

impl Subcommand {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Replay(cmd) => cmd.run(),
            Scan(cmd) => cmd.run(config),
            Show(cmd) => cmd.run(),
        }
    }
}
