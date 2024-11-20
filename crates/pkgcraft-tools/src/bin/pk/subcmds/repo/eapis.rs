use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
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
        for arg in &self.repos {
            let (_pool, repo) = target_ebuild_repo(config, arg)?;
            let mut eapis = HashMap::<_, Vec<_>>::new();
            // TODO: use parallel iterator
            for pkg in repo.iter_raw() {
                let pkg = pkg?;
                eapis.entry(pkg.eapi()).or_default().push(pkg.cpv().clone());
            }

            let mut stdout = io::stdout().lock();
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
