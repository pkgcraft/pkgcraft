use std::process::ExitCode;

mod dep;
mod pkg;
mod version;

use pkgcraft::config::Config;

use crate::Run;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Perform dep-related actions including parsing, intersection, and sorting
    Dep(dep::Command),
    /// Perform package-related actions
    Pkg(pkg::Command),
    /// Perform version-related actions including parsing, intersection, and sorting
    Version(version::Command),
}

impl Run for Subcommand {
    fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Dep(cmd) => cmd.run(config),
            Pkg(cmd) => cmd.run(config),
            Version(cmd) => cmd.run(config),
        }
    }
}
