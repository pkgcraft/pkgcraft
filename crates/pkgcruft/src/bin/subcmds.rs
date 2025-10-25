use std::{path::PathBuf, process::ExitCode};

use strum::AsRefStr;

mod completion;
mod diff;
mod ignore;
mod replay;
mod scan;
mod show;

#[derive(Debug, AsRefStr, clap::Subcommand)]
#[strum(serialize_all = "kebab-case")]
pub(crate) enum Subcommand {
    /// Generate shell completion
    Completion(completion::Command),
    /// Compare reports
    Diff(diff::Command),
    /// Show ignore information
    Ignore(ignore::Command),
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
    pub(super) fn run(&self, cfg: &[PathBuf]) -> anyhow::Result<ExitCode> {
        match self {
            Self::Completion(cmd) => cmd.run(),
            Self::Diff(cmd) => cmd.run(),
            Self::Ignore(cmd) => cmd.run(),
            Self::Replay(cmd) => cmd.run(),
            Self::Scan(cmd) => cmd.run(cfg),
            Self::Show(cmd) => cmd.run(),
        }
    }
}
