use std::process::ExitCode;

mod scan;

use pkgcraft::config::Config;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Scan targets for QA issues
    Scan(scan::Command),
}

impl Subcommand {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Scan(cmd) => cmd.run(config),
        }
    }
}
