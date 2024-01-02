use std::process::ExitCode;

mod cpv;
mod dep;
mod pkg;
mod repo;
mod version;

use pkgcraft::config::Config;

#[derive(Debug, clap::Subcommand)]
pub enum Subcommand {
    /// Cpv commands
    Cpv(cpv::Command),
    /// Dependency commands
    Dep(dep::Command),
    /// Package commands
    Pkg(pkg::Command),
    /// Repository commands
    Repo(repo::Command),
    /// Version commands
    Version(version::Command),
}

impl Subcommand {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        use Subcommand::*;
        match self {
            Cpv(cmd) => cmd.run(),
            Dep(cmd) => cmd.run(),
            Pkg(cmd) => cmd.run(config),
            Repo(cmd) => cmd.run(config),
            Version(cmd) => cmd.run(),
        }
    }
}
