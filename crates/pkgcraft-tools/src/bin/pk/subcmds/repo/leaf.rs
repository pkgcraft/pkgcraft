use std::collections::{HashMap, HashSet};
use std::io::{stdout, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::Intersects;
use pkgcraft::pkg::Package;

use crate::args::target_ebuild_repo;

#[derive(Debug, Args)]
pub struct Command {
    // positionals
    /// Target repository
    #[arg(required = true)]
    repo: String,
}

impl Command {
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        let repo = target_ebuild_repo(config, &self.repo)?;
        let mut cpvs = vec![];
        let mut cache = HashMap::<_, HashSet<_>>::new();

        for pkg in repo.as_ref() {
            let cpv = pkg.cpv();
            cpvs.push(cpv.clone());
            for dep in pkg.dependencies(&[]).iter_flatten() {
                cache
                    .entry(dep.cpn())
                    .or_insert_with(HashSet::new)
                    .insert(dep.clone());
            }
        }

        for cpv in &cpvs {
            // TODO: use is_some_and() once MSRV >= 1.70
            if !cache.get(&cpv.cpn()).map_or(false, |deps| {
                deps.iter()
                    .any(|d| d.intersects(cpv) && d.blocker().is_none())
            }) {
                writeln!(stdout(), "{cpv}")?;
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
