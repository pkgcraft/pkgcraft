use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use indexmap::{IndexMap, IndexSet};
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::{ebuild::raw::Pkg, Source};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::utils::bounded_jobs;
use pkgcraft::Error;
use scallop::pool::PoolIter;
use scallop::variables;

use crate::args::StdinOrArgs;

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
            RepoSet::from_iter([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // original bash variables to remove from the returned envs
        let orig_vars: IndexSet<_> = variables::all_visible().into_iter().collect();

        let func = |pkg: Pkg| -> scallop::Result<(String, IndexMap<String, String>)> {
            // TODO: move error mapping into pkgcraft for pkg sourcing
            pkg.source().map_err(|e| Error::InvalidPkg {
                id: pkg.to_string(),
                err: e.to_string(),
            })?;

            let new_vars: IndexSet<_> = variables::all_visible().into_iter().collect();
            let env: IndexMap<_, _> = new_vars
                .difference(&orig_vars)
                .filter_map(|var| variables::optional(var).map(|val| (var.to_string(), val)))
                .collect();

            Ok((pkg.to_string(), env))
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs.unwrap_or_default());
        let mut status = ExitCode::SUCCESS;
        for target in self.targets.stdin_or_args().split_whitespace() {
            // determine target restriction
            let (repos, restrict) = target_restriction(config, &repos, &target)?;

            // find matching packages from targeted repos
            let pkgs = repos.ebuild().flat_map(|r| r.iter_raw_restrict(&restrict));

            // source ebuilds and output ebuild-specific environment variables
            let mut stderr = io::stderr().lock();
            let mut stdout = io::stdout().lock();
            for result in PoolIter::new(jobs, pkgs, func, true)? {
                match result {
                    Err(e) => {
                        status = ExitCode::FAILURE;
                        writeln!(stderr, "{e}")?;
                    }
                    Ok((pkg, env)) => {
                        writeln!(stdout, "\n{pkg}")?;
                        for (k, v) in env {
                            writeln!(stdout, "{k}={v}")?;
                        }
                    }
                }
            }
        }

        Ok(status)
    }
}
