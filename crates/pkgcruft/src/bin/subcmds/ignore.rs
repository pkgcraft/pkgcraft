use std::io::Write;
use std::process::ExitCode;

use clap::Args;
use clap::builder::ArgPredicate;
use pkgcraft::cli::{MaybeStdinVec, Targets};
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

        // determine package restrictions
        let targets = Targets::new(&mut config)
            .repo_format(RepoFormat::Ebuild)
            .repo(self.repo.as_deref())?
            .pkg_targets(self.targets.iter().flatten())?
            .collapse();

        let mut stdout = anstream::stdout().lock();
        for (repo, restrict) in targets.ebuild_repo_restricts() {
            let ignore = Ignore::new(repo).populate(restrict);
            write!(stdout, "{ignore}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
