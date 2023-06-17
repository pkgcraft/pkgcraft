use std::collections::{HashMap, HashSet};
use std::io::{stdout, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::{Cpv, Intersects};
use pkgcraft::pkg::Package;
use pkgcraft::repo::PkgRepository;

use crate::args::target_ebuild_repo;

#[derive(Debug, Args)]
pub struct Command {
    // positionals
    /// Target repository
    #[arg(required = true)]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repo = target_ebuild_repo(config, &self.repo)?;
        let mut cpvs = vec![];
        let mut cache = HashMap::<_, HashSet<_>>::new();

        for pkg in repo.iter() {
            cpvs.push(pkg.cpv().clone());
            for dep in pkg.dependencies(&[]).iter_flatten() {
                cache
                    .entry(dep.cpn())
                    .or_insert_with(HashSet::new)
                    .insert(dep.clone());
            }
        }

        // determine if a given package is a leaf
        let is_leaf = |cpv: &Cpv| -> bool {
            // TODO: use is_some_and() once MSRV >= 1.70
            !cache.get(&cpv.cpn()).map_or(false, |deps| {
                deps.iter()
                    .any(|d| d.intersects(cpv) && d.blocker().is_none())
            })
        };

        for cpv in cpvs.into_iter().filter(is_leaf) {
            writeln!(stdout(), "{cpv}")?;
        }

        Ok(ExitCode::SUCCESS)
    }
}
