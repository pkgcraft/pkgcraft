use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::{target_restriction, MaybeStdinVec};
use pkgcraft::config::Config;
use pkgcraft::pkg::{ebuild, Pretend};
use pkgcraft::repo::RepoFormat;
use pkgcraft::utils::bounded_jobs;
use scallop::pool::PoolIter;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    // positionals
    /// Target packages or paths
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<MaybeStdinVec<String>>,
}

// TODO: use configured ebuild repos instead of raw ones
// TODO: support binpkg repos
impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let func = |pkg: ebuild::raw::Pkg| -> scallop::Result<Option<String>> {
            let pkg = ebuild::Pkg::try_from(pkg)?;
            pkg.pretend()
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs.unwrap_or_default());
        let mut status = ExitCode::SUCCESS;

        // determine target restrictions
        let targets: Vec<_> = self
            .targets
            .iter()
            .flatten()
            .map(|s| target_restriction(config, Some(RepoFormat::Ebuild), s))
            .try_collect()?;

        // find matching packages from targeted repos
        let pkgs = targets.iter().flat_map(|(repo_set, restrict)| {
            repo_set
                .ebuild()
                .flat_map(move |repo| repo.iter_raw_restrict(restrict))
        });

        // run pkg_pretend across selected pkgs
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());
        for result in PoolIter::new(jobs, pkgs, func, true)? {
            match result {
                Err(e) => {
                    status = ExitCode::FAILURE;
                    writeln!(stderr, "{e}")?;
                }
                Ok(Some(s)) => writeln!(stdout, "{s}")?,
                Ok(None) => (),
            }
        }

        Ok(status)
    }
}
