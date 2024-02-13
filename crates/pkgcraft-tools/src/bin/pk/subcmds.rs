use std::process::ExitCode;

use pkgcraft::config::Config;

mod cpv;
mod dep;
mod pkg;
mod repo;
mod version;

#[derive(Debug, clap::Subcommand)]
pub(crate) enum Subcommand {
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
