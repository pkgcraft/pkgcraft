use std::io::{self, Write};
use std::process::ExitCode;

use clap::{builder::ArgPredicate, Args};
use itertools::Itertools;
use pkgcraft::cli::{ebuild_pkgs, MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
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
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // convert targets to restrictions
        let targets = TargetRestrictions::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_targets(self.targets.iter().flatten())?;

        let mut stdout = io::stdout().lock();
        // convert restrictions to pkgs
        let mut iter = ebuild_pkgs(targets).log_errors();
        for pkg in &mut iter {
            writeln!(stdout, "{pkg}: {}", pkg.keywords().iter().join(" "))?;
        }

        Ok(ExitCode::from(iter))
    }
}
