use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::{Config, Repos};
use pkgcraft::dep::{CpvOrDep, Flatten};
use pkgcraft::traits::Intersects;

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
        let repos = if let Some(target) = self.repo.as_ref() {
            config.add_target_repo(target)?.into()
        } else {
            config.repos.set(Repos::Ebuild)
        };

        // convert targets to Cpv or Dep objects
        let targets: Vec<_> = self.targets.stdin_or_args().split_whitespace().collect();
        let targets: Result<Vec<_>, _> = targets.iter().map(|s| CpvOrDep::parse(s)).collect();
        let targets = targets?;

        // TODO: use a revdeps cache for queries (#120)
        // TODO: parallelize while generating metadata on the fly (#121)
        for repo in repos.ebuild() {
            for pkg in repo {
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
