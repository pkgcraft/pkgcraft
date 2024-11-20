use std::io::{self, Write};
use std::process::ExitCode;

use clap::builder::ArgPredicate;
use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Target repo
    #[arg(long)]
    repo: Option<String>,

    // positionals
    /// Target packages or paths
    #[arg(
        // default to the current working directory
        default_value = ".",
        // default to all packages when targeting a repo
        default_value_if("repo", ArgPredicate::IsPresent, Some("*")),
        help_heading = "Arguments",
    )]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let (_pool, pkgs) = TargetRestrictions::new(config)
            .repo(self.repo)?
            .pkgs_ebuild(self.targets.iter().flatten())?;

        let mut stdout = io::stdout().lock();
        for pkg in pkgs {
            writeln!(stdout, "{pkg}: {}", pkg.keywords().iter().join(" "))?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
