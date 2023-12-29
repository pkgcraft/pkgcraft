use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use indexmap::IndexMap;
use pkgcraft::config::{Config, Repos};
use pkgcraft::pkg::{ebuild::raw::Pkg, Source};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::shell::environment::Variable;
use pkgcraft::shell::metadata::Key;
use pkgcraft::utils::bounded_jobs;
use pkgcraft::Error;
use scallop::pool::PoolIter;
use scallop::variables::{self, ShellVariable};
use strum::IntoEnumIterator;

use crate::args::{multiple_items_iter, StdinOrArgs};

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
    filter: Vec<String>,

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

        let external: HashSet<_> = variables::visible().into_iter().collect();
        let bash: HashSet<_> = ["PIPESTATUS"].into_iter().collect();
        let pms: HashSet<_> = Variable::iter().map(|v| v.to_string()).collect();
        let meta: HashSet<_> = Key::iter().map(|v| v.to_string()).collect();

        // create variable filters
        let (mut hide, mut show) = (HashSet::new(), HashSet::new());
        let items = self.filter.iter().flat_map(|line| line.split(','));
        for item in items {
            // determine filter set
            let (set, var) = match item.strip_prefix('-') {
                Some(var) => (&mut hide, var),
                None => (&mut show, item),
            };

            // expand variable aliases
            match var {
                "@PMS" => set.extend(pms.iter().map(|s| s.as_str())),
                "@META" => set.extend(meta.iter().map(|s| s.as_str())),
                _ => {
                    set.insert(var);
                }
            }
        }

        let filter = |var: &variables::Variable| -> bool {
            let name = var.name();
            !external.contains(name)
                && !bash.contains(name)
                && !hide.contains(name)
                && (show.is_empty() || show.contains(name))
        };

        let value = |var: variables::Variable| -> Option<(String, String)> {
            var.to_vec().map(|v| (var.to_string(), v.join(" ")))
        };

        let func = |pkg: Pkg| -> scallop::Result<(String, IndexMap<String, String>)> {
            // TODO: move error mapping into pkgcraft for pkg sourcing
            pkg.source().map_err(|e| Error::InvalidPkg {
                id: pkg.to_string(),
                err: e.to_string(),
            })?;

            let env: IndexMap<_, _> = variables::visible()
                .into_iter()
                .filter(filter)
                .filter_map(value)
                .collect();

            Ok((pkg.to_string(), env))
        };

        // loop over targets, tracking overall failure status
        let jobs = bounded_jobs(self.jobs.unwrap_or_default());
        let mut status = ExitCode::SUCCESS;

        // determine target restrictions
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| target_restriction(config, &repos, &s))
            .collect();
        let targets = targets?;

        // find matching packages from targeted repos
        let pkgs = targets
            .iter()
            .flat_map(|(repo_set, restrict)| {
                repo_set
                    .ebuild()
                    .flat_map(move |repo| repo.iter_raw_restrict(restrict))
            })
            .peekable();

        // determine if the iterator contains multiple packages
        let (multiple, pkgs) = multiple_items_iter(pkgs);

        // source ebuilds and output ebuild-specific environment variables
        let (mut stdout, mut stderr) = (io::stdout().lock(), io::stderr().lock());
        for result in PoolIter::new(jobs, pkgs, func, true)? {
            match result {
                Err(e) => {
                    status = ExitCode::FAILURE;
                    writeln!(stderr, "{e}")?;
                }
                Ok((pkg, env)) => {
                    if multiple {
                        writeln!(stdout, "\n{pkg}")?;
                    }
                    for (k, v) in env {
                        writeln!(stdout, "{k}={v}")?;
                    }
                }
            }
        }

        Ok(status)
    }
}
