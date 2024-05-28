use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::config::Config;
use pkgcraft::dep::{CpvOrDep, Flatten};
use pkgcraft::repo::RepoFormat;
use pkgcraft::traits::Intersects;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    // positionals
    /// Target dependencies
    #[arg(value_name = "TARGET")]
    targets: Vec<String>,
}

impl Command {
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(target) = self.repo.as_ref() {
            config.add_target_repo(target)?.into()
        } else {
            config.repos.set(Some(RepoFormat::Ebuild))
        };

        // convert targets to Cpv or Dep objects
        let targets: Vec<_> = self.targets.stdin_or_args().split_whitespace().collect();
        let targets: Vec<_> = targets.iter().map(|s| CpvOrDep::try_new(s)).try_collect()?;

        // TODO: use a revdeps cache for queries (#120)
        // TODO: parallelize while generating metadata on the fly (#121)
        let mut stdout = io::stdout().lock();
        for repo in repos.ebuild() {
            for pkg in repo.as_ref() {
                for dep in pkg.dependencies(&[]).into_iter_flatten() {
                    if targets.iter().any(|t| t.intersects(dep)) && dep.blocker().is_none() {
                        writeln!(stdout, "{pkg}: {dep}")?;
                    }
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
