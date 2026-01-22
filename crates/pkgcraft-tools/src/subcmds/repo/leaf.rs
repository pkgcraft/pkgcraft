use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::cli::Targets;
use pkgcraft::config::Config;
use pkgcraft::dep::{Cpv, Flatten};
use pkgcraft::pkg::Package;
use pkgcraft::traits::{Intersects, LogErrors};

#[derive(Args)]
#[clap(next_help_heading = "Leaf options")]
pub(crate) struct Command {
    /// Ignore invalid packages
    #[arg(short, long)]
    ignore: bool,

    // positionals
    /// Target repository
    #[arg(default_value = ".")]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = Targets::new(config)?
            .repo_targets([&self.repo])?
            .ebuild_repo()?;

        let mut cpvs = vec![];
        let mut cache = HashMap::<_, HashSet<_>>::new();

        let mut iter = repo.iter_ordered().log_errors(self.ignore);
        for pkg in &mut iter {
            cpvs.push(pkg.cpv().clone());
            for dep in pkg.dependencies([]).into_iter_flatten() {
                cache
                    .entry(dep.cpn().clone())
                    .or_default()
                    .insert(dep.clone());
            }
        }

        // determine if a given package is a leaf
        let is_leaf = |cpv: &Cpv| -> bool {
            !cache.get(cpv.cpn()).is_some_and(|deps| {
                deps.iter()
                    .any(|d| d.intersects(cpv) && d.blocker().is_none())
            })
        };

        let mut stdout = io::stdout().lock();
        for cpv in cpvs.into_iter().filter(is_leaf) {
            writeln!(stdout, "{cpv}")?;
        }

        Ok(ExitCode::from(iter))
    }
}
