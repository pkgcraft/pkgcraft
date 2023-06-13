use std::io::stdin;
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use is_terminal::IsTerminal;
use itertools::Either;
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::ebuild::{Pkg, RawPkg};
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::RepoFormat::Ebuild as EbuildRepo;
use scallop::pool::PoolIter;

use crate::args::bounded_jobs;

use super::target_restriction;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    // positionals
    /// Target packages or directories
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,
}

// TODO: use configured ebuild repos instead of raw ones
// TODO: support binpkg repos
impl Command {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                EbuildRepo.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // pull targets from args or stdin
        let targets = if stdin().is_terminal() {
            Either::Left(self.targets.into_iter())
        } else {
            Either::Right(stdin().lines().map_while(Result::ok))
        };

        let func = |raw_pkg: RawPkg| -> scallop::Result<()> {
            let pkg: Pkg = raw_pkg.into_pkg()?;
            pkg.pretend()
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs)?;
        let mut failed = false;
        for target in targets {
            // determine target restriction
            let (repos, restrict) = target_restriction(&repos, &target)?;

            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_raw_restrict(&restrict));

            // run pkg_pretend across selected pkgs
            for r in PoolIter::new(jobs, pkgs, func, true)? {
                if let Err(e) = r {
                    failed = true;
                    eprintln!("{e}");
                }
            }
        }

        if failed {
            Ok(ExitCode::FAILURE)
        } else {
            Ok(ExitCode::SUCCESS)
        }
    }
}
