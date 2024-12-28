use std::process::ExitCode;

use crate::command::Command;

mod cpv;
mod dep;
mod pkg;
mod repo;
mod version;

#[derive(clap::Subcommand)]
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
    pub(super) fn run(&self, args: &Command) -> anyhow::Result<ExitCode> {
        match self {
            Self::Cpv(cmd) => cmd.run(),
            Self::Dep(cmd) => cmd.run(),
            Self::Pkg(cmd) => cmd.run(args.load_config()?),
            Self::Repo(cmd) => cmd.run(args.load_config()?),
            Self::Version(cmd) => cmd.run(),
        }
    }
}
