use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::MaybeStdinVec;
use pkgcraft::config::Config;
use pkgcraft::dep::{CpvOrDep, Flatten};
use pkgcraft::traits::{Intersects, LogErrors};

use crate::args::target_ebuild_repo;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Target repository
    #[arg(short, long, value_name = "REPO", default_value = ".")]
    repo: String,

    // positionals
    /// Target dependencies
    #[arg(value_name = "TARGET")]
    targets: Vec<MaybeStdinVec<String>>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = target_ebuild_repo(config, &self.repo)?;
        config.finalize()?;

        // convert targets to Cpv or Dep objects
        let targets: Vec<_> = self
            .targets
            .iter()
            .flatten()
            .map(CpvOrDep::try_new)
            .try_collect()?;

        // TODO: use a revdeps cache for queries (#120)
        // TODO: use parallel iterators (#121)
        let mut stdout = io::stdout().lock();
        let mut iter = repo.iter_unordered().log_errors();
        for pkg in &mut iter {
            for dep in pkg.dependencies([]).into_iter_flatten() {
                if targets.iter().any(|t| t.intersects(dep)) && dep.blocker().is_none() {
                    writeln!(stdout, "{pkg}: {dep}")?;
                }
            }
        }

        Ok(ExitCode::from(iter))
    }
}
