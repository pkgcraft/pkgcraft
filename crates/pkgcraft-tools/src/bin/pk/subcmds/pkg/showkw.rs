use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::target_restriction;
use pkgcraft::config::Config;
use pkgcraft::repo::{PkgRepository, RepoFormat};

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
    // positionals
    /// Target packages or paths
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target restrictions
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| target_restriction(config, Some(RepoFormat::Ebuild), &s))
            .collect();
        let targets = targets?;

        for (repos, restrict) in targets {
            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_restrict(&restrict));

            // TODO: use tabular formatting output
            for pkg in pkgs {
                println!("{pkg}: {}", pkg.keywords().iter().join(" "));
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
