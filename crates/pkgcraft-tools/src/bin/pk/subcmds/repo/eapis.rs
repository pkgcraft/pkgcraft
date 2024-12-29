use std::collections::HashMap;
use std::io::{self, Write};
use std::process::ExitCode;

use clap::Args;
use itertools::Itertools;
use pkgcraft::cli::target_ebuild_repo;
use pkgcraft::config::Config;
use pkgcraft::eapi::{Eapi, EAPIS};
use pkgcraft::pkg::Package;
use pkgcraft::traits::LogErrors;

#[derive(Args)]
pub(crate) struct Command {
    /// Output packages for a target EAPI
    #[arg(short, long)]
    eapi: Option<&'static Eapi>,

    // positionals
    /// Target repositories
    #[arg(value_name = "REPO", default_value = ".")]
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

        let mut failed = false;
        let mut stdout = io::stdout().lock();
        for repo in &repos {
            let mut eapis = HashMap::<_, Vec<_>>::new();

            // TODO: use parallel iterator
            let mut iter = repo.iter_raw().log_errors();
            for pkg in &mut iter {
                eapis.entry(pkg.eapi()).or_default().push(pkg.cpv().clone());
            }
            failed |= iter.failed();

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
                        let s = if cpvs.len() != 1 { "s" } else { "" };
                        writeln!(stdout, "  EAPI {eapi}: {} pkg{s}", cpvs.len())?;
                    }
                }
            }
        }

        Ok(ExitCode::from(failed as u8))
    }
}
