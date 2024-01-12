use std::process::ExitCode;

use pkgcraft::config::Config;
use strum::AsRefStr;

mod replay;
mod scan;
mod show;

#[derive(Debug, AsRefStr, clap::Subcommand)]
#[strum(serialize_all = "snake_case")]
pub enum Subcommand {
    /// Replay reports
    Replay(replay::Command),
    /// Scan for QA issues
    Scan(scan::Command),
    /// Show various information
    Show(show::Command),
}

impl Subcommand {
    pub(super) fn command<'a>(&'a self, cmd: &mut Vec<&'a str>) {
        cmd.push(self.as_ref());
    }
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
