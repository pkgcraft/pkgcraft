use std::io::{self, Write};
use std::process::ExitCode;

use clap::builder::ArgPredicate;
use clap::Args;
use pkgcraft::cli::{MaybeStdinVec, TargetRestrictions};
use pkgcraft::config::Config;
use pkgcraft::repo::RepoFormat;
use pkgcruft::ignore::Ignore;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Ignore options")]
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
    pub(super) fn run(&self) -> anyhow::Result<ExitCode> {
        let mut config = Config::new("pkgcraft", "");

        // determine target restrictions
        let targets = TargetRestrictions::new(&mut config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .finalize_targets(self.targets.iter().flatten())?;

        let mut stdout = io::stdout().lock();
        for (repo, restrict) in targets.ebuild_repo_restricts() {
            let ignore = Ignore::new(repo);
            ignore.populate(restrict);
            write!(stdout, "{ignore}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
