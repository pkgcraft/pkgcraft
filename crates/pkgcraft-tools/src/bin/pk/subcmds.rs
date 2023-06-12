use std::process::ExitCode;

mod dep;
mod pkg;
mod repo;
mod version;

use pkgcraft::config::Config;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Perform dep-related actions including parsing, intersection, and sorting
    Dep(dep::Command),
    /// Perform package-related actions
    Pkg(pkg::Command),
    /// Perform repo-related actions
    Repo(repo::Command),
    /// Perform version-related actions including parsing, intersection, and sorting
    Version(version::Command),
}

impl Subcommand {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Dep(cmd) => cmd.run(config),
            Pkg(cmd) => cmd.run(config),
            Repo(cmd) => cmd.run(config),
            Version(cmd) => cmd.run(config),
        }
    }
}
