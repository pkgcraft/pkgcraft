use std::io::{stderr, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::ebuild::{Pkg, RawPkg};
use pkgcraft::pkg::BuildablePackage;
use pkgcraft::repo::set::RepoSet;
use scallop::pool::PoolIter;

use crate::args::{bounded_jobs, stdin_or_args};

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
    pub(super) fn run(self, config: &mut Config) -> anyhow::Result<ExitCode> {
        // determine target repo set
        let repos = if let Some(repo) = self.repo.as_ref() {
            let repo = if let Some(r) = config.repos.get(repo) {
                Ok(r.clone())
            } else if Path::new(repo).exists() {
                config.add_nested_repo_path(repo, 0, repo, true)
            } else {
                anyhow::bail!("unknown repo: {repo}")
            }?;
            RepoSet::new([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        let func = |raw_pkg: RawPkg| -> scallop::Result<()> {
            let pkg: Pkg = raw_pkg.into_pkg()?;
            pkg.pretend()
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs)?;
        let mut status = ExitCode::SUCCESS;
        for target in stdin_or_args(self.targets) {
            // determine target restriction
            let (repos, restrict) = target_restriction(config, &repos, &target)?;

            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_raw_restrict(&restrict));

            // run pkg_pretend across selected pkgs
            for r in PoolIter::new(jobs, pkgs, func, true)? {
                if let Err(e) = r {
                    status = ExitCode::FAILURE;
                    writeln!(stderr(), "{e}")?;
                }
            }
        }

        Ok(status)
    }
}
