use std::io::{self, Write};
use std::process::ExitCode;

use clap::{builder::ArgPredicate, Args};
use itertools::Itertools;
use pkgcraft::cli::{MaybeStdinVec, Targets};
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
#[clap(next_help_heading = "Target options")]
pub(crate) struct Command {
    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

    /// Target repo
    #[arg(short, long)]
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
        // convert targets to pkgs
        let mut iter = Targets::new(config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_pkgs(self.targets.iter().flatten())?
            .ebuild_pkgs()
            .log_errors(self.ignore);

        let mut stdout = io::stdout().lock();
        for pkg in &mut iter {
            writeln!(stdout, "{pkg}: {}", pkg.keywords().iter().join(" "))?;
        }

        Ok(ExitCode::from(iter))
    }
}
