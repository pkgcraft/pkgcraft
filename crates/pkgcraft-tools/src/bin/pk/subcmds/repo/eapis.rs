use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::config::Config;
use pkgcraft::eapi::{Eapi, EAPIS};
use pkgcraft::pkg::Package;

use crate::args::target_ebuild_repo;

#[derive(Debug, Args)]
pub(crate) struct Command {
    /// Output packages for a target EAPI
    #[arg(short, long)]
    eapi: Option<&'static Eapi>,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", required = true)]
    repos: Vec<String>,
}

impl Command {
    pub(super) fn run(&self, config: &mut Config) -> anyhow::Result<ExitCode> {
        let repos: Vec<_> = self
            .repos
            .iter()
            .map(|x| target_ebuild_repo(config, x))
            .try_collect()?;
        config.finalize()?;

        let mut stdout = io::stdout().lock();
        for repo in &repos {
            let mut eapis = HashMap::<_, Vec<_>>::new();
            // TODO: use parallel iterator
            for pkg in repo.iter_raw() {
                let pkg = pkg?;
                eapis.entry(pkg.eapi()).or_default().push(pkg.cpv().clone());
            }

            if let Some(eapi) = self.eapi {
                if let Some(cpvs) = eapis.get_mut(eapi) {
                    cpvs.sort();
                    for cpv in cpvs {
                        writeln!(stdout, "{cpv}")?;
                    }
                }
            } else if !eapis.is_empty() {
                writeln!(stdout, "{repo}")?;
                for eapi in &*EAPIS {
                    if let Some(cpvs) = eapis.get(eapi) {
                        writeln!(stdout, "  EAPI {eapi}: {} pkgs", cpvs.len())?;
                    }
                }
            }
        }

        Ok(ExitCode::SUCCESS)
    }
}
