use std::path::Path;
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::dep::{CpvOrDep, Flatten, Intersects};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::PkgRepository;

use crate::args::StdinOrArgs;

#[derive(Debug, Args)]
pub struct Command {
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

        // convert targets to Cpv or Dep objects
        let targets: Result<Vec<CpvOrDep>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| s.parse())
            .collect();
        let targets = targets?;

        // TODO: use a revdeps cache for queries (#120)
        // TODO: parallelize while generating metadata on the fly (#121)
        for repo in repos.ebuild() {
            for pkg in repo.iter() {
                for dep in pkg.dependencies(&[]).into_iter_flatten() {
                    if targets.iter().any(|t| t.intersects(dep)) && dep.blocker().is_none() {
                        println!("{pkg}: {dep}");
                    }
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
