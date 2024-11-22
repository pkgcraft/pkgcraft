use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::{Cpv, Flatten};
use pkgcraft::pkg::Package;
use pkgcraft::traits::Intersects;

use crate::args::target_ebuild_repo;

#[derive(Debug, Args)]
pub(crate) struct Command {
    // positionals
    /// Target repository
    #[arg(required = true)]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = target_ebuild_repo(config, &self.repo)?;
        config.finalize()?;

        let mut cpvs = vec![];
        let mut cache = HashMap::<_, HashSet<_>>::new();

        for pkg in repo.iter_unordered() {
            let pkg = pkg?;
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

        Ok(ExitCode::SUCCESS)
    }
}
