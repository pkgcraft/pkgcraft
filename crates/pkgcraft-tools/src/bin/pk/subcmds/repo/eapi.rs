use std::collections::HashMap;
use std::io::{stdout, Write};
use std::process::ExitCode;

use clap::Args;
use pkgcraft::config::Config;
use pkgcraft::dep::Cpv;
use pkgcraft::eapi::{Eapi, EAPIS};
use pkgcraft::pkg::Package;

use crate::args::target_ebuild_repos;

#[derive(Debug, Args)]
pub struct Command {
    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", required = true)]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &Config) -> anyhow::Result<ExitCode> {
        for repo in target_ebuild_repos(config, &self.repos)? {
            let mut eapis = HashMap::<&'static Eapi, Vec<Cpv>>::new();
            // TODO: use parallel iterator
            for pkg in repo.iter_raw() {
                eapis
                    .entry(pkg.eapi())
                    .or_insert_with(Vec::new)
                    .push(pkg.cpv().clone());
            }

            writeln!(stdout(), "{repo}:")?;
            for eapi in EAPIS.iter() {
                if let Some(cpvs) = eapis.get(eapi) {
                    writeln!(stdout(), "  EAPI {eapi}: {} pkgs", cpvs.len())?;
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
