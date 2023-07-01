use std::path::Path;
use std::process::ExitCode;
use std::str::FromStr;

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::dep::{CpvOrDep, Intersects};
use pkgcraft::repo::set::RepoSet;
use pkgcraft::repo::PkgRepository;
use rayon::prelude::*;

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
            RepoSet::new([&repo])
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // convert targets to Cpv or Dep objects
        let targets: Result<Vec<_>, _> = self
            .targets
            .stdin_or_args()
            .split_whitespace()
            .map(|s| CpvOrDep::from_str(&s))
            .collect();
        let targets = targets?;

        // TODO: use a revdeps cache for queries
        for repo in repos.ebuild() {
            let cpvs: Vec<_> = repo.iter_cpv().collect();
            // iterate over cpvs in parallel looking for reverse deps
            cpvs.par_iter().for_each(|cpv| {
                let pkg = repo.iter_restrict(cpv).next().unwrap();
                for dep in pkg.dependencies(&[]).iter_flatten() {
                    if targets.iter().any(|t| t.intersects(dep)) && dep.blocker().is_none() {
                        println!("{pkg}: {dep}");
                    }
                }
            });
        }

        Ok(ExitCode::SUCCESS)
    }
}
