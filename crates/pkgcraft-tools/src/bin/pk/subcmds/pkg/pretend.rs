use std::io::stdin;
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use is_terminal::IsTerminal;
use itertools::Either;
use pkgcraft::config::{Config, RepoSetType};
use pkgcraft::pkg::ebuild::{Pkg, RawPkg};
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::RepoFormat;
use pkgcraft::restrict;
use scallop::pool::PoolIter;

use crate::args::bounded_jobs;

#[derive(Debug, Args)]
pub struct Command {
    /// Parallel jobs to run
    #[arg(short, long)]
    jobs: Option<usize>,

    /// Target repository
    #[arg(short, long)]
    repo: Option<String>,

    // positionals
    /// Target packages
    #[arg(value_name = "PKG")]
    vals: Vec<String>,
}

impl Command {
    pub(super) fn run(self, config: &Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let reposet = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                RepoFormat::Ebuild.load_from_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(RepoSetType::Ebuild)
        };

        // TODO: use configured ebuild repos instead of raw ones
        // TODO: support binpkg repos
        // restrict searches to ebuild repos
        let repos: Vec<_> = reposet
            .repos()
            .iter()
            .filter_map(|r| r.as_ebuild())
            .collect();
        if repos.is_empty() {
            anyhow::bail!("no ebuild repos found");
        }

        // pull targets from args or stdin
        let args = if stdin().is_terminal() {
            Either::Left(self.vals.into_iter())
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
        for target in args {
            let restrict = restrict::parse::dep(&target)?;

            // convert repos into packages
            let pkgs = repos.iter().flat_map(|r| r.iter_raw_restrict(&restrict));

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
