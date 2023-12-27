use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use indexmap::{IndexMap, IndexSet};
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::{ebuild::raw::Pkg, Source};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::shell::environment::Variable;
use pkgcraft::shell::metadata::Key;
use pkgcraft::utils::bounded_jobs;
use pkgcraft::Error;
use scallop::pool::PoolIter;
use scallop::variables;
use strum::IntoEnumIterator;

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

    /// Variable filtering
    #[arg(short, long)]
    filter: Option<String>,

    // positionals
    /// Target packages or directories
    #[arg(value_name = "TARGET", default_value = ".")]
    targets: Vec<String>,
}

// TODO: support other repo types such as configured and binpkg
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

        // external variables to remove
        let orig_vars: IndexSet<_> = variables::all_visible().into_iter().collect();
        let pms_vars: IndexSet<_> = Variable::iter().map(|v| v.to_string()).collect();
        let meta_vars: IndexSet<_> = Key::iter().map(|v| v.to_string()).collect();

        // create variable filters
        let (mut hide, mut show) = (IndexSet::new(), IndexSet::new());
        if let Some(filter) = &self.filter {
            for var in filter.split(',') {
                if let Some(v) = var.strip_prefix('-') {
                    match v {
                        "PMS" => hide.extend(pms_vars.iter().map(|s| s.as_str())),
                        "META" => hide.extend(meta_vars.iter().map(|s| s.as_str())),
                        _ => {
                            hide.insert(v);
                        }
                    }
                } else {
                    match var {
                        "PMS" => show.extend(pms_vars.iter().map(|s| s.as_str())),
                        "META" => show.extend(meta_vars.iter().map(|s| s.as_str())),
                        _ => {
                            show.insert(var);
                        }
                    }
                }
            }
        }

        let filter_func = |var: &String| -> bool {
            let var = var.as_str();
            !orig_vars.contains(var)
                && !hide.contains(var)
                && (show.is_empty() || show.contains(var))
        };

        let func = |pkg: Pkg| -> scallop::Result<(String, IndexMap<String, String>)> {
            // TODO: move error mapping into pkgcraft for pkg sourcing
            pkg.source().map_err(|e| Error::InvalidPkg {
                id: pkg.to_string(),
                err: e.to_string(),
            })?;

            let env: IndexMap<_, _> = variables::all_visible()
                .into_iter()
                .filter(filter_func)
                .filter_map(|var| variables::optional(&var).map(|val| (var, val)))
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
