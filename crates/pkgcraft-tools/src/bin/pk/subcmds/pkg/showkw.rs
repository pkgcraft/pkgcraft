use std::io::{self, Write};
use std::process::ExitCode;

use clap::builder::ArgPredicate;
use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::TargetRestrictions;
use pkgcraft::config::Config;
use pkgcraft::repo::PkgRepository;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
#[clap(next_help_heading = "Target options")]
pub struct Command {
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
    targets: Vec<String>,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target restrictions
        let targets = TargetRestrictions::new(config)
            .repo(self.repo)?
            .targets(self.targets.stdin_or_args().split_whitespace())?;

        let mut stdout = io::stdout().lock();
        for (repo_set, restricts) in targets {
            // find matching packages from targeted repos
            let pkgs = restricts.iter().flat_map(|r| {
                repo_set
                    .ebuild()
                    .flat_map(move |repo| repo.iter_restrict(r))
            });

            // TODO: use tabular formatting output
            for pkg in pkgs {
                writeln!(stdout, "{pkg}: {}", pkg.keywords().iter().join(" "))?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
